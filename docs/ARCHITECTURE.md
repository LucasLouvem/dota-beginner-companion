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

| Módulo           | Responsabilidade                                                                                                         | Pode depender de        |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------- |
| `domain`         | Regras puras: formato KeyValues/VDF, conteúdo e validação do `.cfg` do GSI, instalação do Dota, relatório de diagnóstico | nada do projeto         |
| `application`    | Ports, mensagens (`CoreEvent`, `UiCommand`), estado observável (`AppSnapshot`), laço do núcleo e casos de uso            | `domain`                |
| `gsi`            | Servidor GSI (Axum) e cliente do teste de conexão                                                                        | `domain`, `application` |
| `infrastructure` | Adaptadores do sistema local: bibliotecas Steam, processos, arquivos de configuração GSI, `config.toml`                  | `domain`, `application` |
| `presentation`   | Interface GTK4/Relm4 e traduções; exibe o estado e envia comandos                                                        | `domain`, `application` |
| `lib.rs`         | Declara as camadas como biblioteca, usada por `main.rs` e pelos testes de integração                                     | todos                   |
| `main.rs`        | Composição: lê a configuração, cria os adaptadores, inicia o runtime e abre a janela                                     | todos                   |

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

Nos testes, os casos de uso recebem implementações falsas desses ports.

## Fluxo de eventos

1. O Dota envia um POST JSON para `http://127.0.0.1:<porta>/`.
2. O servidor valida o token e o JSON e publica um `CoreEvent`: aceito, recusado, servidor ouvindo ou servidor com falha.
3. O laço do núcleo (`application::core`) aplica o evento ao `GsiStatus` e publica um novo `AppSnapshot`.
4. A interface recebe cada `AppSnapshot` e atualiza as telas.
5. Os botões da interface enviam `UiCommand`: rodar diagnóstico, testar conexão ou instalar a configuração GSI. O núcleo executa o caso de uso correspondente e publica o resultado no `AppSnapshot`.

O diagnóstico também roda automaticamente a cada 10 segundos. Os eventos de teste ficam separados dos eventos do Dota no estado, para que um teste bem-sucedido não seja exibido como conexão com o jogo.

## Concorrência

- `main.rs` cria um runtime tokio próprio. O servidor GSI e o laço do núcleo rodam nele.
- A thread do GTK só executa a interface. Ela recebe o estado por `watch` dentro de um comando Relm4.
- Operações de arquivo e de `/proc` rodam em `spawn_blocking`, e o teste de conexão roda em uma tarefa separada. A interface nunca espera por I/O.
- Quando o log de eventos está ativo (`DOTA_COMPANION_GSI_LOG`), uma única tarefa grava os eventos do Dota um de cada vez, na ordem de chegada. Eventos de teste e requisições recusadas não são gravados.

## Segurança do servidor GSI

- Escuta somente em `127.0.0.1`. O endereço é fixo e a porta é configurável.
- Exige o token do `config.toml`, comparado em tempo constante.
- Recusa corpos acima de 1 MB.
- Remove o bloco `auth` antes de guardar ou exibir o JSON.
- O `config.toml` é gravado com permissão `0600`.

## Decisões aprovadas e não implementadas

Decisões de arquitetura já aprovadas que ainda não existem no código. Quando uma delas é implementada, ela sai desta tabela e passa a ser descrita nas seções acima.

| Camada          | Decisão                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `storage`       | Persistência local em SQLite para cache, configurações e histórico de partidas                                                                                                                                                                                                                                                                                                                                                                                |
| `providers`     | A aplicação depende de uma interface de provider, não de uma API específica. Ordem de leitura: dado novo da API, depois cache vencido marcado como antigo, depois dados embutidos no binário                                                                                                                                                                                                                                                                  |
| `notifications` | Notificações do sistema, desligadas por padrão (opt-in)                                                                                                                                                                                                                                                                                                                                                                                                       |
| `application`   | Histórico de partidas gravado e lido por meio da camada `storage`                                                                                                                                                                                                                                                                                                                                                                                             |
| `application`   | Contrato único de diagnóstico em `application/error.rs`, com `code`, `title`, `cause`, `resolution` e `technical_details` opcional. Erros específicos preservam a origem; causas desconhecidas usam `UNCLASSIFIED_ERROR`, conforme [ERRORS.md](ERRORS.md). O domínio permanece independente da aplicação, que converte seus erros para o contrato. A interface renderiza o diagnóstico e códigos HTTP ficam restritos ao adaptador HTTP.                      |
| Testes          | Unitários fora de `src/`, em `tests/unit/`, espelhando os módulos de produção e vinculados como módulos filhos com `#[cfg(test)]` e `#[path]`. Auxiliares ficam em `tests/support/`. Integração usa `tests/integration_tests.rs` e módulos em `tests/integration/`; E2E usa `tests/e2e_tests.rs`. A API para integração será exposta por `src/lib.rs`, preservando o encapsulamento. Organização e verificação de resultados seguem [TESTING.md](TESTING.md). |
