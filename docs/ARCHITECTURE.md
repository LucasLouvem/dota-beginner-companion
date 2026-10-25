# Arquitetura

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-24

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

| Módulo           | Responsabilidade                                                                                                                               | Pode depender de        |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| `domain`         | Regras puras: KeyValues/VDF, `.cfg` do GSI, instalação do Dota, estado de jogo, modo de jogo, eventos de partida e catálogos de heróis e itens | nada do projeto         |
| `application`    | Ports, mensagens, estado (`AppSnapshot`), relatório e contrato de diagnóstico (`error`), laço do núcleo e casos de uso                         | `domain`                |
| `gsi`            | Servidor GSI (Axum), normalização do JSON em estado de jogo e cliente do teste de conexão                                                      | `domain`, `application` |
| `infrastructure` | Adaptadores do sistema local: bibliotecas Steam, processos, arquivos GSI, `config.toml` e dados embutidos de heróis e itens                    | `domain`, `application` |
| `storage`        | Banco local SQLite (cache com prazo de validade)                                                                                               | `domain`, `application` |
| `presentation`   | Interface GTK4/Relm4 e traduções; exibe o estado e envia comandos                                                                              | `domain`, `application` |
| `lib.rs`         | Declara as camadas como biblioteca, usada por `main.rs` e pelos testes de integração                                                           | todos                   |
| `main.rs`        | Composição: lê a configuração, cria os adaptadores, inicia o runtime e abre a janela                                                           | todos                   |

Regras:

- `domain` e `application` não importam GTK, Axum, Reqwest nem acessam arquivos diretamente.
- `presentation` não contém regras de negócio. Ela só transforma `AppSnapshot` em widgets e cliques em `UiCommand`.
- Adaptadores não conhecem uns aos outros. A ligação entre eles é feita em `main.rs`.

## Ports e adaptadores

| Port (`application::ports`) | Adaptador                       | Função                                                                  |
| --------------------------- | ------------------------------- | ----------------------------------------------------------------------- |
| `DotaLocator`               | `infrastructure::steam_locator` | Lista as bibliotecas Steam e as instalações do Dota 2                   |
| `ProcessProbe`              | `infrastructure::process`       | Informa se há processo `dota2` em `/proc/*/comm`                        |
| `ConfigFiles`               | `infrastructure::config_files`  | Lê, grava (de forma atômica) e faz backup dos arquivos `.cfg`           |
| `ConnectionTester`          | `gsi::client`                   | Envia um evento de teste ao servidor GSI local                          |
| `GsiRecorder`               | `infrastructure::gsi_log_file`  | Grava os eventos do Dota em arquivo JSON Lines (opcional)               |
| `CacheStore`                | `storage::sqlite_cache`         | Grava e lê o cache local; valores vencidos voltam marcados como antigos |

O `CacheStore` ainda não é usado pelo núcleo: os dados que ele vai guardar vêm dos providers da Fase 3. Os dados dos heróis (`infrastructure::hero_data`) e dos itens (`infrastructure::item_data`) são recortes do dotaconstants embutidos no binário, com fonte e licença em `assets/dotaconstants/SOURCE.md`.

Nos testes, os casos de uso recebem implementações falsas desses ports. Os testes unitários ficam em `tests/unit/` e os de integração em `tests/integration/`, conforme [TESTING.md](TESTING.md).

## Fluxo de eventos

1. O Dota envia um POST JSON para `http://127.0.0.1:<porta>/`.
2. O servidor valida o token e o JSON e publica um `CoreEvent`: aceito, recusado, servidor ouvindo ou servidor com falha. Um evento aceito do Dota leva também o estado de jogo normalizado (`gsi::normalize`); cada campo é lido isoladamente, e um bloco vazio (`{}`) significa "sem dado".
3. O laço do núcleo (`application::core`) aplica o evento ao `GsiStatus`, passa o estado de jogo ao `MatchTracker` (`domain::match_events`) e publica um novo `AppSnapshot` com o estado atual, a linha do tempo da partida e o resumo da última partida terminada, que fica disponível até a próxima partida ser encontrada.
4. A interface lê o `AppSnapshot` mais recente e atualiza nas telas só as linhas que mudaram. Na aba Herói, eventos novos entram no topo da linha do tempo e os mais antigos saem de baixo, sem recriar as demais linhas.
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

| Camada          | Decisão                                                                                                                                                                                      |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `storage`       | Configurações e histórico de partidas no SQLite (o cache já existe; ver acima)                                                                                                               |
| `providers`     | A aplicação depende de uma interface de provider, não de uma API específica. Ordem de leitura: dado novo da API, depois cache vencido marcado como antigo, depois dados embutidos no binário |
| `notifications` | Notificações do sistema, desligadas por padrão (opt-in)                                                                                                                                      |
| `application`   | Histórico de partidas gravado e lido por meio da camada `storage`                                                                                                                            |
| Testes          | Testes ponta a ponta (E2E) em `tests/e2e_tests.rs`, exercitando o aplicativo completo pelas interfaces externas, conforme [TESTING.md](TESTING.md)                                           |
