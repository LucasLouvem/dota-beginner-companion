# Fixtures do GSI

Eventos do Dota 2 usados nos testes do normalizador e dos eventos de partida. Cada linha segue o formato do log de desenvolvimento: `{"received_at_ms": <unix ms>, "payload": <JSON do GSI sem o bloco auth>}`.

- `zeus_bot_match.jsonl`: partida real contra bots, do menu até o início da partida. Cobre menu sem `map`, herói com `id: 0`, escolha do herói, compras na estratégia, relógio voltando e inventário a partir do pré-jogo.
- `dragon_knight_bot_match.jsonl`: partida real contra bots, já em andamento. Cobre subida de nível, ponto de habilidade, item entrando no inventário, item movido do stash e morte.
- `synthetic_post_game.jsonl`: **sintético**, escrito à mão. Representa o fim de partida (`POST_GAME`) com `win_team`, que ainda não foi capturado do jogo, e não comprova o formato real.

Os arquivos reais foram recortados do log local, com eventos escolhidos e completos. Antes de entrar no repositório, `player.steamid`, `player.accountid` e `player.name` (também dentro de `previously`) foram trocados por valores fictícios fixos. Nenhum arquivo daqui pode conter dados pessoais.
