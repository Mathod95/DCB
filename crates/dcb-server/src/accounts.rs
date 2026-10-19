//! Connexion, déconnexion, « Mon compte » et administration des comptes.
//!
//! Les comptes sont créés uniquement par un admin (pas d'inscription). Garde-fous : un admin ne
//! peut pas se retirer ses propres droits, se désactiver ou se supprimer, et le dernier admin
//! actif ne peut pas disparaître.

use std::time::Duration;

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use serde::Deserialize;

use crate::auth::{self, AdminUser, CurrentUser, MaybeUser, Role, User};
use crate::views::*;
use crate::web::{AppResult, AppState, render};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_page).post(login))
        .route("/logout", post(logout))
        .route("/account", get(account_page).post(change_password))
        .route("/admin/users", get(admin_users).post(admin_create))
        .route("/admin/users/{id}/role", post(admin_role))
        .route("/admin/users/{id}/active", post(admin_active))
        .route("/admin/users/{id}/password", post(admin_password))
        .route("/admin/users/{id}/delete", post(admin_delete))
}

// ---------------------------------------------------------------------------------------------
// Connexion

#[derive(Deserialize, Default)]
struct NextQuery {
    next: Option<String>,
}

async fn login_page(
    State(s): State<AppState>,
    MaybeUser(user): MaybeUser,
    Query(q): Query<NextQuery>,
) -> AppResult<Response> {
    let next = auth::safe_next(q.next.as_deref());
    if user.is_some() {
        return Ok(Redirect::to(&next).into_response());
    }
    let no_users = auth::count_users(&s.app_db).await? == 0;
    Ok(render(LoginPage {
        ctx: Ctx::anonymous(),
        next,
        username: String::new(),
        error: None,
        no_users,
    })?
    .into_response())
}

#[derive(Deserialize)]
struct LoginForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    next: String,
}

async fn login(State(s): State<AppState>, Form(f): Form<LoginForm>) -> AppResult<Response> {
    let next = auth::safe_next(Some(&f.next));
    match auth::check_login(&s.app_db, &f.username, &f.password).await? {
        Some(user) => {
            let token = auth::create_session(&s.app_db, user.id).await?;
            tracing::info!("connexion de {}", user.username);
            let mut res = Redirect::to(&next).into_response();
            res.headers_mut().insert(
                header::SET_COOKIE,
                auth::session_cookie(&token, s.secure_cookies),
            );
            Ok(res)
        }
        None => {
            // Ralentit les essais en série ; le hachage Argon2 coûte déjà plusieurs dizaines de ms.
            tokio::time::sleep(Duration::from_millis(400)).await;
            tracing::warn!("échec de connexion pour {:?}", f.username.trim());
            let page = LoginPage {
                ctx: Ctx::anonymous(),
                next,
                username: f.username.trim().to_owned(),
                error: Some("Identifiant ou mot de passe incorrect.".into()),
                no_users: false,
            };
            Ok((StatusCode::UNAUTHORIZED, render(page)?).into_response())
        }
    }
}

async fn logout(State(s): State<AppState>, req: axum::extract::Request) -> AppResult<Response> {
    let (parts, _) = req.into_parts();
    if let Some(token) = auth::session_token(&parts) {
        auth::delete_session(&s.app_db, &token).await?;
    }
    let mut res = Redirect::to("/login").into_response();
    res.headers_mut()
        .insert(header::SET_COOKIE, auth::clear_cookie(s.secure_cookies));
    Ok(res)
}

// ---------------------------------------------------------------------------------------------
// Mon compte

async fn account_page(
    State(s): State<AppState>,
    CurrentUser(u): CurrentUser,
) -> AppResult<Response> {
    Ok(render(AccountPage {
        ctx: s.ctx(&u),
        role_label: u.role.label(),
        error: None,
        done: false,
    })?
    .into_response())
}

#[derive(Deserialize)]
struct PasswordChange {
    #[serde(default)]
    current: String,
    #[serde(default)]
    new: String,
    #[serde(default)]
    confirm: String,
}

async fn change_password(
    State(s): State<AppState>,
    CurrentUser(u): CurrentUser,
    Form(f): Form<PasswordChange>,
) -> AppResult<Response> {
    let error = if auth::check_login(&s.app_db, &u.username, &f.current)
        .await?
        .is_none()
    {
        Some("Le mot de passe actuel est incorrect.".to_owned())
    } else if f.new != f.confirm {
        Some("Les deux nouveaux mots de passe ne correspondent pas.".to_owned())
    } else {
        auth::validate_password(&f.new)
            .err()
            .map(|e| format!("Nouveau mot de passe : {e}."))
    };
    if let Some(error) = error {
        let page = AccountPage {
            ctx: s.ctx(&u),
            role_label: u.role.label(),
            error: Some(error),
            done: false,
        };
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, render(page)?).into_response());
    }

    auth::set_password(&s.app_db, u.id, &f.new).await?;
    // Déconnecte les autres appareils, garde celui-ci connecté avec une nouvelle session.
    auth::delete_sessions_of(&s.app_db, u.id).await?;
    let token = auth::create_session(&s.app_db, u.id).await?;
    let mut res = render(AccountPage {
        ctx: s.ctx(&u),
        role_label: u.role.label(),
        error: None,
        done: true,
    })?
    .into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        auth::session_cookie(&token, s.secure_cookies),
    );
    Ok(res)
}

// ---------------------------------------------------------------------------------------------
// Administration des comptes

async fn users_page(
    s: &AppState,
    me: &User,
    notice: Option<Notice>,
    new_username: String,
    status: StatusCode,
) -> AppResult<Response> {
    let users = auth::list_users(&s.app_db).await?;
    let page = AdminUsersPage {
        ctx: s.ctx(me),
        users: users.iter().map(|u| UserRowView::new(u, me)).collect(),
        notice,
        new_username,
        editor_enabled: s.editor_open(),
        sandbox: s.sandbox,
    };
    Ok((status, render(page)?).into_response())
}

fn ok(text: impl Into<String>) -> Option<Notice> {
    Some(Notice {
        ok: true,
        text: text.into(),
        password: None,
    })
}

fn err(text: impl Into<String>) -> Option<Notice> {
    Some(Notice {
        ok: false,
        text: text.into(),
        password: None,
    })
}

async fn admin_users(State(s): State<AppState>, AdminUser(me): AdminUser) -> AppResult<Response> {
    users_page(&s, &me, None, String::new(), StatusCode::OK).await
}

#[derive(Deserialize)]
struct NewUser {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    role: String,
}

async fn admin_create(
    State(s): State<AppState>,
    AdminUser(me): AdminUser,
    Form(f): Form<NewUser>,
) -> AppResult<Response> {
    let role = Role::parse(&f.role).unwrap_or(Role::User);
    // Champ vide : on génère un mot de passe temporaire à transmettre à la personne.
    let generated = f.password.is_empty();
    let password = if generated {
        auth::generate_password()
    } else {
        f.password.clone()
    };
    match auth::create_user(&s.app_db, &f.username, &password, role).await? {
        Ok(_) => {
            let name = auth::validate_username(&f.username).unwrap_or_default();
            tracing::info!("{} a créé le compte {name} ({})", me.username, role.slug());
            let notice = Some(Notice {
                ok: true,
                text: format!("Compte « {name} » créé ({}).", role.label()),
                password: generated.then_some(password),
            });
            users_page(&s, &me, notice, String::new(), StatusCode::OK).await
        }
        Err(e) => {
            users_page(
                &s,
                &me,
                err(capitalize(&e)),
                f.username,
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str() + ".",
        None => String::new(),
    }
}

/// Compte visé par une action d'admin, avec les garde-fous communs : il doit exister, et un
/// admin ne modifie pas son propre compte ici (il passe par « Mon compte »).
async fn target(s: &AppState, me: &User, id: i64) -> AppResult<Result<User, Option<Notice>>> {
    let Some(u) = auth::find_user(&s.app_db, id).await? else {
        return Ok(Err(err("Ce compte n'existe plus.")));
    };
    if u.id == me.id {
        return Ok(Err(err(
            "Tu ne peux pas modifier ton propre compte ici. Utilise « Mon compte ».",
        )));
    }
    Ok(Ok(u))
}

/// Le dernier admin actif ne peut pas perdre ses droits.
async fn would_remove_last_admin(s: &AppState, u: &User) -> AppResult<bool> {
    Ok(u.is_admin() && u.active && auth::count_active_admins(&s.app_db).await? <= 1)
}

#[derive(Deserialize)]
struct RoleForm {
    role: String,
}

async fn admin_role(
    State(s): State<AppState>,
    AdminUser(me): AdminUser,
    Path(id): Path<i64>,
    Form(f): Form<RoleForm>,
) -> AppResult<Response> {
    let u = match target(&s, &me, id).await? {
        Ok(u) => u,
        Err(n) => {
            return users_page(&s, &me, n, String::new(), StatusCode::UNPROCESSABLE_ENTITY).await;
        }
    };
    let Some(role) = Role::parse(&f.role) else {
        return users_page(
            &s,
            &me,
            err("Rôle inconnu."),
            String::new(),
            StatusCode::BAD_REQUEST,
        )
        .await;
    };
    if role == Role::User && would_remove_last_admin(&s, &u).await? {
        return users_page(
            &s,
            &me,
            err("Il doit rester au moins un admin actif."),
            String::new(),
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    }
    auth::set_role(&s.app_db, id, role).await?;
    users_page(
        &s,
        &me,
        ok(format!(
            "« {} » est maintenant {}.",
            u.username,
            role.label()
        )),
        String::new(),
        StatusCode::OK,
    )
    .await
}

async fn admin_active(
    State(s): State<AppState>,
    AdminUser(me): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let u = match target(&s, &me, id).await? {
        Ok(u) => u,
        Err(n) => {
            return users_page(&s, &me, n, String::new(), StatusCode::UNPROCESSABLE_ENTITY).await;
        }
    };
    if u.active && would_remove_last_admin(&s, &u).await? {
        return users_page(
            &s,
            &me,
            err("Il doit rester au moins un admin actif."),
            String::new(),
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    }
    auth::set_active(&s.app_db, id, !u.active).await?;
    let text = if u.active {
        format!("« {} » est désactivé et a été déconnecté.", u.username)
    } else {
        format!("« {} » est réactivé.", u.username)
    };
    users_page(&s, &me, ok(text), String::new(), StatusCode::OK).await
}

async fn admin_password(
    State(s): State<AppState>,
    AdminUser(me): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let u = match target(&s, &me, id).await? {
        Ok(u) => u,
        Err(n) => {
            return users_page(&s, &me, n, String::new(), StatusCode::UNPROCESSABLE_ENTITY).await;
        }
    };
    let password = auth::generate_password();
    auth::set_password(&s.app_db, id, &password).await?;
    auth::delete_sessions_of(&s.app_db, id).await?;
    let notice = Some(Notice {
        ok: true,
        text: format!(
            "Nouveau mot de passe pour « {} ». Ses sessions ont été fermées.",
            u.username
        ),
        password: Some(password),
    });
    users_page(&s, &me, notice, String::new(), StatusCode::OK).await
}

async fn admin_delete(
    State(s): State<AppState>,
    AdminUser(me): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let u = match target(&s, &me, id).await? {
        Ok(u) => u,
        Err(n) => {
            return users_page(&s, &me, n, String::new(), StatusCode::UNPROCESSABLE_ENTITY).await;
        }
    };
    if would_remove_last_admin(&s, &u).await? {
        return users_page(
            &s,
            &me,
            err("Il doit rester au moins un admin actif."),
            String::new(),
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    }
    auth::delete_user(&s.app_db, id).await?;
    tracing::info!("{} a supprimé le compte {}", me.username, u.username);
    users_page(
        &s,
        &me,
        ok(format!("Compte « {} » supprimé.", u.username)),
        String::new(),
        StatusCode::OK,
    )
    .await
}
