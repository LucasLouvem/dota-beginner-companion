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

| Módulo           | Responsabilidade                                                                                                       | Pode depender de        |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| `domain`         | Regras puras: formato KeyValues/VDF, conteúdo e validação do `.cfg` do GSI, instalação do Dota                         | nada do projeto         |
| `application`    | Ports, mensagens, estado (`AppSnapshot`), relatório e contrato de diagnóstico (`error`), laço do núcleo e casos de uso | `domain`                |
| `gsi`            | Servidor GSI (Axum) e cliente do teste de conexão                                                                      | `domain`, `application` |
| `infrastructure` | Adaptadores do sistema local: bibliotecas Steam, processos, arquivos de configuração GSI, `config.toml`                | `domain`, `application` |
| `presentation`   | Interface GTK4/Relm4 e traduções; exibe o estado e envia comandos                                                      | `domain`, `application` |
| `lib.rs`         | Declara as camadas como biblioteca, usada por `main.rs` e pelos testes de integração                                   | todos                   |
| `main.rs`        | Composição: lê a configuração, cria os adaptadores, inicia o runtime e abre a janela                                   | todos                   |

Regras:

- `domain` e `application` não importam GTK, Axum, Reqwest nem acessam arquivos diretamente.
- `presentation` não contém regras de negócio. Ela só transforma `AppSnapshot` em widgets e cliques em `UiCommand`.
- Adaptadores não conhecem uns aos outros. A ligação entre eles é feita em `main.rs`.

## Ports e adaptadores

| Port (`application::ports`) | Adaptador                       | Função                                                        |
| --------------------------- | ------------------------------- | ------------------------------------------------------------- |
| `DotaLocator`               | `infrastructure::steam_locator` | Lista as bibliotecas Steam e as instalações do Dota 2         |
| `ProcessProbe`              | `infrastructure::process`       | Informa se há processo `dota2` em `/proc/*/comm`              |
| `ConfigFiles`               | `infrastructure::config_files`  | Lê, grava (de forma atômica) e faz backup dos arquivos `.cfg` |
| `ConnectionTester`          | `gsi::client`                   | Envia um evento de teste ao servidor GSI local                |
| `GsiRecorder`               | `infrastructure::gsi_log_file`  | Grava os eventos do Dota em arquivo JSON Lines (opcional)     |

Nos testes, os casos de uso recebem implementações falsas desses ports. Os testes unitários ficam em `tests/unit/` e os de integração em `tests/integration/`, conforme [TESTING.md](TESTING.md).

## Fluxo de eventos

1. O Dota envia um POST JSON para `http://127.0.0.1:<porta>/`.
2. O servidor valida o token e o JSON e publica um `CoreEvent`: aceito, recusado, servidor ouvindo ou servidor com falha.
3. O laço do núcleo (`application::core`) aplica o evento ao `GsiStatus` e publica um novo `AppSnapshot`.
4. A interface recebe cada `AppSnapshot` e atualiza as telas.
5. Os botões da interface enviam `UiCommand`: rodar diagnóstico, testar conexão ou instalar a configuração GSI. O núcleo executa o caso de uso correspondente e publica o resultado no `AppSnapshot`.

O diagnóstico também roda automaticamente a cada 10 segundos. Os eventos de teste ficam separados dos eventos do Dota no estado, para que um teste bem-sucedido não seja exibido como conexão com o jogo.

## Erros e diagnóstico

- Cada camada tem erros tipados que guardam a causa original. Os que vão para o `AppSnapshot` guardam a causa em `Arc`, para o estado continuar `Clone`.
- O domínio não conhece o contrato de diagnóstico. A `application` converte os erros dele, e cada erro específico implementa `ToDiagnostic` (`application::error`).
- A interface converte o erro em `Diagnostic` no momento de exibir, com o `Redactor` do `AppSnapshot`, que remove o token. A mesma linha (`diagnostic_row`) mostra título, causa, resolução, código e detalhes técnicos.
- Falhas que impedem a inicialização encerram o aplicativo com o mesmo diagnóstico no terminal, sem a cadeia completa do erro.
- Códigos HTTP só existem no adaptador `gsi`. Detalhes e catálogo em [ERRORS.md](ERRORS.md).

## Concorrência

- `main.rs` cria um runtime tokio próprio. O servidor GSI e o laço do núcleo rodam nele.
- A thread do GTK só executa a interface. Ela recebe o estado por `watch` dentro de um comando Relm4.
- Operações de arquivo e de `/proc` rodam em `spawn_blocking`, e o teste de conexão roda em uma tarefa separada. A interface nunca espera por I/O.
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
| `storage`       | Persistência local em SQLite para cache, configurações e histórico de partidas                                                                                                               |
| `providers`     | A aplicação depende de uma interface de provider, não de uma API específica. Ordem de leitura: dado novo da API, depois cache vencido marcado como antigo, depois dados embutidos no binário |
| `notifications` | Notificações do sistema, desligadas por padrão (opt-in)                                                                                                                                      |
| `application`   | Histórico de partidas gravado e lido por meio da camada `storage`                                                                                                                            |
| Testes          | Testes ponta a ponta (E2E) em `tests/e2e_tests.rs`, exercitando o aplicativo completo pelas interfaces externas, conforme [TESTING.md](TESTING.md)                                           |
