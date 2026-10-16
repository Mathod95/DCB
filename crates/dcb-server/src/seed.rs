//! Cartes de démonstration, reprises des textes du Figma DevCardBattle.
//! Insérées uniquement au premier démarrage, quand la table est vide (désactivable avec DCB_SEED=0).
//! Les coûts sont des valeurs provisoires, à équilibrer.

use dcb_core::{CardDraft, CardType, Rarity, Skin};
use sqlx::SqlitePool;

use crate::db;

struct Seed {
    name: &'static str,
    card_type: CardType,
    rarity: Rarity,
    description: &'static str,
    effect: &'static str,
    deploy_cost: i64,
    daily_cost: i64,
    flavor_text: &'static str,
}

const SEEDS: &[Seed] = &[
    Seed {
        name: "ARGO CD",
        card_type: CardType::App,
        rarity: Rarity::Gold,
        description: "Permet de structurer vos repository Git et de déployer des resources Argo CD",
        effect: "- Débloque la carte applicationSet\n- Ajoute +1 deployment MAX\n- Gestion multi environment (deployment unifié)",
        deploy_cost: 3,
        daily_cost: 1,
        flavor_text: "Pas de Flux dans ce jeu !",
    },
    Seed {
        name: "DDOS",
        card_type: CardType::Incident,
        rarity: Rarity::Basic,
        description: "Une attaque DDoS massive surcharge vos serveurs.",
        effect: "- Inflige 5 dégâts\n- Ralentit les réponses de 70%",
        deploy_cost: 4,
        daily_cost: 0,
        flavor_text: "",
    },
    Seed {
        name: "ZOMBIE PROCESSES",
        card_type: CardType::Incident,
        rarity: Rarity::Basic,
        description: "Des processus zombies s'accumulent sur vos serveurs.",
        effect: "- Consomme 2 CPU par tour\n- Durée 4 tours",
        deploy_cost: 2,
        daily_cost: 0,
        flavor_text: "",
    },
    Seed {
        name: "MEMORY LEAK",
        card_type: CardType::Incident,
        rarity: Rarity::Basic,
        description: "Une fuite mémoire consomme progressivement la RAM.",
        effect: "- Inflige 2 dégâts par tour pendant 3 tours",
        deploy_cost: 2,
        daily_cost: 0,
        flavor_text: "",
    },
    Seed {
        name: "REGISTRY DOWN",
        card_type: CardType::Incident,
        rarity: Rarity::Gold,
        description: "Impossible de pull les images Docker.",
        effect: "- Empêche tout déploiement pendant 2 tours",
        deploy_cost: 5,
        daily_cost: 0,
        flavor_text: "",
    },
    Seed {
        name: "AUTO-SCALING",
        card_type: CardType::Resource,
        rarity: Rarity::Basic,
        description: "Augmente automatiquement vos ressources selon la charge.",
        effect: "- Réduit les dégâts de 50%",
        deploy_cost: 2,
        daily_cost: 1,
        flavor_text: "",
    },
    Seed {
        name: "HEALTH CHECK",
        card_type: CardType::Resource,
        rarity: Rarity::Basic,
        description: "Détecte et remplace automatiquement les instances défaillantes.",
        effect: "- Soigne 3 points de vie\n- Réduit l'instabilité de 10%",
        deploy_cost: 1,
        daily_cost: 0,
        flavor_text: "",
    },
    Seed {
        name: "LOAD BALANCER",
        card_type: CardType::Resource,
        rarity: Rarity::Basic,
        description: "Distribue intelligemment le trafic entre vos serveurs.",
        effect: "- Immunité contre les cartes de ralentissement\n- Durée 3 tours",
        deploy_cost: 3,
        daily_cost: 1,
        flavor_text: "",
    },
    Seed {
        name: "MONITORING ALERT",
        card_type: CardType::Resource,
        rarity: Rarity::Basic,
        description: "Prévient d'un problème avant qu'il empire.",
        effect: "- Révèle la prochaine carte de l'adversaire\n- Piochez 1 carte",
        deploy_cost: 1,
        daily_cost: 0,
        flavor_text: "",
    },
    Seed {
        name: "ROLLBACK",
        card_type: CardType::Resource,
        rarity: Rarity::Gold,
        description: "Retour instantané à la version stable.",
        effect: "- Annule la dernière carte jouée (y compris la vôtre)",
        deploy_cost: 4,
        daily_cost: 0,
        flavor_text: "",
    },
    Seed {
        name: "HN FRONT PAGE",
        card_type: CardType::Event,
        rarity: Rarity::Basic,
        description: "Votre app est sur la front page de Hacker News.",
        effect: "- +1000% trafic pendant 1 tour\n- +5$ si vous survivez",
        deploy_cost: 0,
        daily_cost: 0,
        flavor_text: "",
    },
    Seed {
        name: "SRE D'ASTREINTE",
        card_type: CardType::Hr,
        rarity: Rarity::Basic,
        description: "Toujours joignable, même à 3h du matin.",
        effect: "- Résout 1 incident en cours",
        deploy_cost: 2,
        daily_cost: 2,
        flavor_text: "",
    },
];

/// Renvoie le nombre de cartes insérées (0 si la base contenait déjà des cartes).
pub async fn run_if_empty(pool: &SqlitePool) -> anyhow::Result<usize> {
    if db::count(pool).await? > 0 {
        return Ok(0);
    }
    for (i, s) in SEEDS.iter().enumerate() {
        let draft = CardDraft {
            name: s.name.into(),
            card_type: s.card_type,
            // Alterne les skins pour montrer les deux habillages dans la galerie.
            skin: if i % 4 == 3 { Skin::Dark } else { Skin::Light },
            rarity: s.rarity,
            description: s.description.into(),
            effect: s.effect.into(),
            deploy_cost: s.deploy_cost,
            daily_cost: s.daily_cost,
            flavor_text: s.flavor_text.into(),
            art_path: None,
        }
        .validated()
        .map_err(|e| anyhow::anyhow!("seed {:?} invalide: {e:?}", s.name))?;
        db::insert(pool, &draft).await?;
    }
    Ok(SEEDS.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_seed_is_valid() {
        for s in SEEDS {
            let d = CardDraft {
                name: s.name.into(),
                card_type: s.card_type,
                skin: Skin::Light,
                rarity: s.rarity,
                description: s.description.into(),
                effect: s.effect.into(),
                deploy_cost: s.deploy_cost,
                daily_cost: s.daily_cost,
                flavor_text: s.flavor_text.into(),
                art_path: None,
            };
            assert!(d.validated().is_ok(), "{}", s.name);
        }
    }
}
