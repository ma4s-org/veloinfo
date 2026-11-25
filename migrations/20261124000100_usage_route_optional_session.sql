-- Un itinéraire peut être enregistré par le serveur lorsque le navigateur
-- n'a pas pu envoyer sa session de suivi (JavaScript resté en cache, requête
-- interrompue). On conserve alors la demande d'itinéraire sans inventer une
-- session qui fausserait les statistiques de visites.

alter table usage_route
    alter column session_id drop not null;
