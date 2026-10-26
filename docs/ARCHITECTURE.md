# Arquitetura

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-26

Como o aplicativo está organizado e como os dados circulam entre as partes.

## Visão geral

O aplicativo é um **monólito modular orientado a eventos** e segue os princípios de Clean Architecture/Hexagonal. Ele é um único binário: as camadas são módulos do mesmo crate, separados por responsabilidade e comunicando-se por mensagens e traits.

```text
Dota 2
  │  POST JSON (GSI)
  ▼
gsi ── servidor HTTP em 127.0.0.1, valida o token
  │  CoreEvent (tokio mpsc)
  ▼
application ── laço do núcleo + casos de uso
  │  AppSnapshot (tokio watch)          ▲ UiCommand (tokio mpsc)
  ▼                                     │
presentation ── janela GTK4/Relm4 ──────┘
```

Os casos de uso acessam o sistema (Steam, processos, arquivos, HTTP) somente por **ports**, que são traits definidas em `application`. Os **adaptadores** em `infrastructure` e `gsi` implementam essas traits.

## Camadas

| Módulo           | Responsabilidade                                                                                                                                                                                          | Pode depender de        |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| `domain`         | Regras puras: KeyValues/VDF, `.cfg` do GSI, instalação do Dota, estado de jogo, modo de jogo, eventos de partida e catálogos de heróis, itens, IDs de habilidades e patches, e a build real de um jogador | nada do projeto         |
| `application`    | Ports, mensagens, estado (`AppSnapshot`), relatório e contrato de diagnóstico (`error`), laço do núcleo e casos de uso                                                                                    | `domain`                |
| `gsi`            | Servidor GSI (Axum), normalização do JSON em estado de jogo e cliente do teste de conexão                                                                                                                 | `domain`, `application` |
| `infrastructure` | Adaptadores do sistema local: bibliotecas Steam, processos, arquivos GSI, `config.toml`, cache das imagens dos itens e dados embutidos de heróis, itens, IDs de habilidades e patches                     | `domain`, `application` |
| `storage`        | Banco local SQLite com schema versionado e migrações atômicas (`storage::schema`): cache com prazo de validade e tabelas de orçamento de chamadas, vitórias de partidas públicas e cursor da coleta       | `domain`, `application` |
| `providers`      | Dados externos: cliente HTTP de JSON (`providers::http_json`) com tempo limite, limite de tamanho, intervalo mínimo, nova tentativa no HTTP 429 e reserva no orçamento antes de cada tentativa            | `domain`, `application` |
| `presentation`   | Interface GTK4/Relm4 e traduções; exibe o estado e envia comandos                                                                                                                                         | `domain`, `application` |
| `lib.rs`         | Declara as camadas como biblioteca, usada por `main.rs` e pelos testes de integração                                                                                                                      | todos                   |
| `main.rs`        | Composição: lê a configuração, cria os adaptadores, inicia o runtime e abre a janela                                                                                                                      | todos                   |

Regras:

- `domain` e `application` não importam GTK, Axum, Reqwest nem acessam arquivos diretamente.
- `presentation` não contém regras de negócio. Ela só transforma `AppSnapshot` em widgets e cliques em `UiCommand`.
- Adaptadores não conhecem uns aos outros. A ligação entre eles é feita em `main.rs`.

## Ports e adaptadores

| Port (`application::ports`) | Adaptador                         | Função                                                                                                                                                                                                                                             |
| --------------------------- | --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DotaLocator`               | `infrastructure::steam_locator`   | Lista as bibliotecas Steam e as instalações do Dota 2                                                                                                                                                                                              |
| `ProcessProbe`              | `infrastructure::process`         | Informa se há processo `dota2` em `/proc/*/comm`                                                                                                                                                                                                   |
| `ConfigFiles`               | `infrastructure::config_files`    | Lê, grava (de forma atômica) e faz backup dos arquivos `.cfg`                                                                                                                                                                                      |
| `ConnectionTester`          | `gsi::client`                     | Envia um evento de teste ao servidor GSI local                                                                                                                                                                                                     |
| `GsiRecorder`               | `infrastructure::gsi_log_file`    | Grava os eventos do Dota em arquivo JSON Lines (opcional)                                                                                                                                                                                          |
| `CacheStore`                | `storage::sqlite_cache`           | Grava e lê o cache local; valores vencidos voltam marcados como antigos                                                                                                                                                                            |
| `CallBudget`                | `storage::call_budget`            | Reserva cada chamada ao OpenDota no orçamento do aplicativo, por dia e por mês, de forma atômica                                                                                                                                                   |
| `StatsProvider`             | `providers::opendota`             | Busca dados do OpenDota como texto JSON (para o cache) e os decodifica campo a campo: patches, partidas recentes de um herói, detalhes de partidas (reduzidos aos campos usados, sem contas e nomes de jogadores) e a amostra de partidas públicas |
| `PublicBuildStore`          | `storage::public_builds`          | Grava as vitórias de cada partida pública junto com o cursor da coleta, numa única transação; lê as mais recentes por herói, modo e patch e apaga as de outros patches                                                                             |
| `ItemIconSource`            | `providers::steam_cdn`            | Baixa a imagem de um item do CDN da Steam, só da pasta de itens do Dota, com tempo limite, limite de 64 KB, até 4 downloads ao mesmo tempo e assinatura PNG conferida                                                                              |
| `ItemIconStore`             | `infrastructure::item_icon_cache` | Guarda as imagens dos itens em `~/.cache/dota-companion/item-icons/`, com gravação atômica                                                                                                                                                         |

O orçamento de chamadas ao OpenDota é uma política conservadora do próprio aplicativo, não um limite oficial do OpenDota (`application::use_cases::call_budget`): por padrão, 1.000 chamadas por dia e 30.000 por mês, contadas em UTC. A coleta em segundo plano pode usar até 700 por dia e 21.000 por mês; as consultas das telas só param nos totais, então a coleta nunca ocupa as 300 chamadas diárias reservadas a elas. Cada reserva confere e incrementa os contadores numa transação `IMMEDIATE` do SQLite.

Dados externos seguem o fallback de `application::use_cases::fallback`: cache ainda válido (sem chamar a API nem gastar orçamento), dado novo da API (gravado no cache só se for decodificado), cache vencido marcado como antigo, dados embutidos quando existem para a consulta e, sem nenhum deles, "sem dados" com a causa. As recomendações são identificadas por herói, modo e patch, com uma geração crescente (`application::use_cases::recommendation_request`): só a resposta da consulta atual é aceita.

As vitórias profissionais (`application::use_cases::pro_builds`) partem de `/heroes/{hero_id}/matches`, que a documentação do OpenDota não descreve como lista de partidas profissionais: só contam partidas de liga (`leagueid` diferente de zero), conferido também nos detalhes. São examinadas até 60 candidatas, da mais recente para a mais antiga, até achar 3 vitórias do herói no patch atual (do jogador, ou do topo da partida) com inventário. Derrotas indicadas pela lista são puladas sem buscar os detalhes. Cada vitória guarda a build real e a ordem de habilidades, que só existe em partida analisada. Cada consulta tem um `QueryBudget` de 40 chamadas, que conta a lista, os detalhes, novas tentativas e respostas 429; o cache não conta. Cada descarte tem um motivo tipado, registrado no log.

A coleta de partidas públicas (`application::use_cases::public_collection`) roda numa tarefa do runtime iniciada pelo `main.rs`, enquanto o aplicativo está aberto, só com o orçamento da categoria dela. Cada ciclo lê uma amostra de `/publicMatches` a partir do cursor, aceita só Escolha Livre (`game_mode` 1 e 22) e Turbo (23) fora dos lobbies de bots (`lobby_type` 4 e 10), busca os detalhes de cada partida aceita e grava a build real de cada vencedor do patch atual. A lista pública não traz os heróis, que vêm dos detalhes. O cursor anda para partidas mais antigas e volta às mais recentes quando a amostra acaba ou chega a outro patch. Sem orçamento, a coleta espera 15 minutos; depois de uma falha, 5 minutos. Ao mudar o patch atual, as vitórias de outros patches são apagadas. As recomendações mostram as 5 vitórias mais recentes do herói nos `game_mode` do modo escolhido.

O `CacheStore` ainda não é usado pelo núcleo: os dados que ele vai guardar vêm dos providers da Fase 3. Os dados dos heróis (`infrastructure::hero_data`), dos itens com custo e receita (`infrastructure::item_data`), dos IDs das habilidades (`infrastructure::ability_id_data`) e dos patches (`infrastructure::patch_data`) são recortes do dotaconstants embutidos no binário, com fonte e licença em `assets/dotaconstants/SOURCE.md`.

`domain::builds` descreve a build real de um jogador numa partida, como o OpenDota informa: o inventário final (`item_0..5`), a mochila (`backpack_0..2`), o item neutro e o aprimoramento dele (`item_neutral` e `item_neutral2`) e, em partida analisada, todas as compras (`purchase_log`) em ordem de horário, inclusive consumíveis e itens repetidos. O aplicativo não agrupa partidas nem escolhe ou esconde itens; só um ID de item que o catálogo embutido não conhece fica de fora. Sem nenhum espaço do inventário na resposta, o jogador não tem build. A versão 3 do banco trocou a tabela das builds públicas de três itens pela de vitórias com a build real; as linhas antigas e os detalhes de partida guardados no cache sem a mochila e o neutro são descartados na migração.

`domain::build_progress` acompanha uma build real pela partida atual: ordena os itens finais pela primeira compra (sem registro de compras, pela ordem dos espaços), separa os itens iniciais, agrupa todas as compras por janela de tempo e marca cada item e cada ponto de habilidade como feito, próximo ou pendente. Um item montado conta como compra dos componentes, pelas receitas do catálogo. O OpenDota não informa o nível do herói em cada ponto de habilidade (só `ability_upgrades_arr`), então o progresso compara com os pontos gastos que o GSI informa: a soma dos níveis das habilidades e os talentos escolhidos (`hero.talent_1..8`). Cada build recomendada tem uma identidade estável (`BuildId`: fonte e partida); a escolha da build fica só na interface e é mantida enquanto herói, modo e patch não mudam.

As imagens dos itens (`application::use_cases::item_icons`) usam o caminho `img` do recorte embutido do dotaconstants. O núcleo pede as imagens do inventário atual e, quando chegam recomendações novas, as de todas as builds, cada item uma única vez por execução, numa tarefa fora do laço; as imagens carregadas vão para o `AppSnapshot` num mapa compartilhado (`Arc`). Uma falha só deixa o item com o nome em texto.

Nos testes, os casos de uso recebem implementações falsas desses ports. Os testes unitários ficam em `tests/unit/` e os de integração em `tests/integration/`, conforme [TESTING.md](TESTING.md).

## Fluxo de eventos

1. O Dota envia um POST JSON para `http://127.0.0.1:<porta>/`.
2. O servidor valida o token e o JSON e publica um `CoreEvent`: aceito, recusado, servidor ouvindo ou servidor com falha. Um evento aceito do Dota leva também o estado de jogo normalizado (`gsi::normalize`); cada campo é lido isoladamente, e um bloco vazio (`{}`) significa "sem dado".
3. O laço do núcleo (`application::core`) aplica o evento ao `GsiStatus`, passa o estado de jogo ao `MatchTracker` (`domain::match_events`) e publica um novo `AppSnapshot` com o estado atual, a linha do tempo da partida e o resumo da última partida terminada, que fica disponível até a próxima partida ser encontrada. Com as fontes externas disponíveis, o núcleo também mantém as recomendações (`RecommendationStatus`): carrega o patch atual ao iniciar e, a cada mudança do herói (da partida atual, ou da última enquanto ela é mostrada), do modo ou do patch, inicia uma consulta em uma tarefa; só a resposta da consulta atual é publicada, e a de uma consulta substituída é descartada e registrada no log.
4. A interface lê o `AppSnapshot` mais recente e atualiza nas telas só as linhas que mudaram. Na página da build (aba Herói), a lista de vitórias só é refeita quando as recomendações mudam; a cada evento do GSI só o progresso da build escolhida é recalculado, e as faixas de itens e a grade de habilidades mudam só os elementos alterados. Na aba Herói, eventos novos entram no topo da linha do tempo e os mais antigos saem de baixo, sem recriar as demais linhas.
5. Os botões da interface enviam `UiCommand`: rodar diagnóstico, testar conexão, instalar a configuração GSI, marcar "Partida encontrada" ou informar o modo de jogo. O núcleo executa o caso de uso correspondente e publica o resultado no `AppSnapshot`.

O diagnóstico também roda automaticamente a cada 10 segundos. Os eventos de teste ficam separados dos eventos do Dota no estado, para que um teste bem-sucedido não seja exibido como conexão com o jogo, e não passam pelo `MatchTracker`.

O `MatchTracker` usa a ordem de chegada (o `clock_time` do Dota volta entre fases), guarda o que já aconteceu na partida para não repetir eventos quando a fase volta e marca cada evento como confirmado pelo GSI, inferido ou manual. Uma partida nova começa na primeira fase antes do jogo (espera dos jogadores ou seleção de heróis) vinda do menu ou do fim de outra partida. Itens que entram ou saem são sempre inferidos: o espaço do TP é ignorado, itens do espaço neutro são marcados como neutros e, quando entra um item cuja receita (do catálogo de itens) inclui itens que saíram na mesma mudança, o evento é de item montado. No pós-jogo, o `MatchSummary` resume a partida a partir do último estado.

O GSI não informa o modo de jogo nem o momento em que a partida é aceita. Cada "Partida encontrada", automática ou manual, recebe um número (`match_number`) e abre a pergunta do modo, com o último modo confirmado na execução pré-selecionado (Escolha Livre na primeira partida). A interface abre a pergunta uma vez por número. Só o botão "Confirmar" grava o modo como informado pelo usuário; fechar a pergunta grava o modo pré-selecionado como modo padrão. O comando de escolha leva o número da partida, e o núcleo ignora a resposta de uma partida anterior. O modo fica só em memória e acompanha a última partida enquanto ela é mostrada.

## Erros e diagnóstico

- Cada camada tem erros tipados que guardam a causa original. Os que vão para o `AppSnapshot` guardam a causa em `Arc`, para o estado continuar `Clone`.
- O domínio não conhece o contrato de diagnóstico. A `application` converte os erros dele, e cada erro específico implementa `ToDiagnostic` (`application::error`).
- A interface converte o erro em `Diagnostic` no momento de exibir, com o `Redactor` do `AppSnapshot`, que remove o token. A mesma linha (`RowContent::diagnostic`) mostra título, causa, resolução, código e detalhes técnicos.
- Falhas que impedem a inicialização encerram o aplicativo com o mesmo diagnóstico no terminal, sem a cadeia completa do erro.
- Antes de ler a configuração ou criar threads, o executável instala um hook de pânico em `infrastructure::logging`. Ele escreve diretamente em stderr uma mensagem fixa e o nome do arquivo, linha e coluna, sem payload, nome de thread ou backtrace. Não invoca o hook padrão nem altera a propagação do pânico.
- O exemplo Rust `send_test_event`, chamado pelo script de teste manual, reutiliza a configuração e o cliente GSI da biblioteca. Envia um único evento marcado como teste, sem iniciar o servidor ou a interface.
- Códigos HTTP só existem no adaptador `gsi`. Detalhes e catálogo em [ERRORS.md](ERRORS.md).

## Concorrência

- `main.rs` cria um runtime tokio próprio. O servidor GSI e o laço do núcleo rodam nele.
- A configuração global de idioma (`setlocale`) roda uma única vez na inicialização, antes do runtime Tokio e do GTK, sem outras threads consultando ou alterando o locale. Esse uso de `unsafe` foi aprovado pelo mantenedor.
- A thread do GTK só executa a interface. Um comando Relm4 acompanha o `watch` e avisa a janela quando há estado novo, com no máximo um aviso pendente; ao tratar o aviso, a janela lê o estado mais recente. Vários estados publicados antes de a janela tratar o aviso resultam em uma única atualização.
- Operações de arquivo e de `/proc` rodam em `spawn_blocking`, e o teste de conexão roda em uma tarefa separada. A interface nunca espera por I/O.
- O diagnóstico automático, o manual e o feito depois da instalação são rodadas numeradas numa única sequência e rodam em paralelo. Só a rodada iniciada por último publica: o resultado ou a interrupção de uma rodada mais antiga é descartado, para não aparecer por cima de um mais novo. A instalação publica sempre o próprio resultado; o diagnóstico que ela gera segue a mesma regra.
- Quando o log de eventos está ativo (`DOTA_COMPANION_GSI_LOG`), uma única tarefa grava os eventos do Dota um de cada vez, na ordem de chegada. Eventos de teste e requisições recusadas não são gravados.
- O núcleo acompanha o fim de cada tarefa (diagnóstico, instalação, teste de conexão e gravação). Se uma delas entrar em pânico ou for cancelada, a interrupção aparece no `AppSnapshot` e no log, sem a mensagem do pânico.
- O laço do núcleo não termina sozinho; o encerramento normal é o `shutdown_background` do runtime depois que a janela fecha. Por isso, um canal fechado com o aplicativo aberto é sempre falha. O servidor responde 503 quando não consegue entregar um evento aceito, e a interface mostra "O núcleo do aplicativo parou" quando perde o estado ou um comando.
- O `.env` é lido antes do log e das outras configurações, sem alterar o ambiente do processo: os valores dele só entram pela busca de variáveis usada por `config::load_or_init` e pelo filtro do `RUST_LOG`.

## Segurança do servidor GSI

- Escuta somente em `127.0.0.1`. O endereço é fixo e a porta é configurável.
- Exige o token do `config.toml`, comparado em tempo constante.
- Recusa corpos acima de 1 MB.
- Remove o bloco `auth` antes de guardar ou exibir o JSON.
- O `config.toml` é gravado com permissão `0600`.

## Decisões aprovadas e não implementadas

Decisões de arquitetura já aprovadas que ainda não existem no código. Quando uma delas é implementada, ela sai desta tabela e passa a ser descrita nas seções acima.

| Camada          | Decisão                                                                                                                                            |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `storage`       | Configurações e histórico de partidas no SQLite (o cache já existe; ver acima)                                                                     |
| `notifications` | Notificações do sistema, desligadas por padrão (opt-in)                                                                                            |
| `application`   | Histórico de partidas gravado e lido por meio da camada `storage`                                                                                  |
| Testes          | Testes ponta a ponta (E2E) em `tests/e2e_tests.rs`, exercitando o aplicativo completo pelas interfaces externas, conforme [TESTING.md](TESTING.md) |
