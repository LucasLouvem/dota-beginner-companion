# Fixtures do GSI

Eventos do Dota 2 usados nos testes do normalizador e dos eventos de partida. Cada linha segue o formato do log de desenvolvimento: `{"received_at_ms": <unix ms>, "payload": <JSON do GSI sem o bloco auth>}`.

- `zeus_bot_match.jsonl`: partida real contra bots, do menu até o início da partida. Cobre menu sem `map`, herói com `id: 0`, escolha do herói, compras na estratégia, relógio voltando e inventário a partir do pré-jogo.
- `dragon_knight_bot_match.jsonl`: partida real contra bots, já em andamento. Cobre subida de nível, ponto de habilidade, item entrando no inventário, item movido do stash e morte.
- `dragon_knight_match_end.jsonl`: fim real de uma partida contra bots. Cobre a montagem do Satanic a partir dos componentes, o primeiro `POST_GAME` com `win_team` e o `matchid` ainda `"0"`, o `POST_GAME` seguinte com o ID da partida e a volta ao menu.

Os arquivos reais foram recortados do log local, com eventos escolhidos e completos. Antes de entrar no repositório, `player.steamid`, `player.accountid` e `player.name` (também dentro de `previously`) foram trocados por valores fictícios fixos. O ID real da partida também foi trocado por um ID fictício (`1234567890`), porque ele permite encontrar a partida e a conta do jogador em serviços públicos. Nenhum arquivo daqui pode conter dados pessoais.
