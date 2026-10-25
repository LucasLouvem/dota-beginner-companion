# Respostas do OpenDota

**Criado em:** 2026-10-25 · **Atualizado em:** 2026-10-25

Respostas reais da API pública do OpenDota (`https://api.opendota.com/api`), usadas pelos testes dos decodificadores em `src/providers/opendota.rs`. Os arquivos não têm dados pessoais: contas e nomes de jogadores são removidos antes de versionar.

| Arquivo                  | Consulta                  | Obtido em  | Observação                                                                                                                                                                          |
| ------------------------ | ------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `constants_patch.json`   | `GET /constants/patch`    | 2026-10-25 | Sem alteração; 61 patches, o último é 7.41 (id 60)                                                                                                                                  |
| `heroes_22_matches.json` | `GET /heroes/22/matches`  | 2026-10-25 | 100 partidas do Zeus; `account_id` removido. A resposta real tem `account_id`, `player_slot`, abates, mortes e assistências, e não tem os nomes dos times listados na especificação |
| `match_9055224065.json`  | `GET /matches/9055224065` | 2026-10-25 | Partida de liga do patch 7.41, analisada. Reduzida aos campos que o app guarda (os mesmos de `reduce_match`), sem contas e nomes de jogadores                                       |
| `public_matches.json`    | `GET /publicMatches`      | 2026-10-25 | Sem alteração; 100 partidas. Os heróis dos times (`radiant_team`, `dire_team`) vêm zerados; 81 turbo, 14 Escolha Livre (1 e 22) e 10 com `lobby_type` 4 (contra bots)               |
