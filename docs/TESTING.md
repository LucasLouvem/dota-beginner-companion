# Testes automatizados

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-24

Regras e organização dos testes do projeto.

## Regras

- **Testes são obrigatórios.** Todo comportamento novo ou alterado vem com testes no mesmo commit.
- Cada comportamento tem **caso de sucesso e caso de erro**. Exemplos: token válido e token inválido; arquivo criado e falha de permissão; arquivo VDF válido e malformado.
- Os testes não dependem de rede externa, do Dota instalado, da Steam do desenvolvedor nem da ordem de execução.
- Os testes não alteram arquivos fora de diretórios temporários.
- Um teste verifica um comportamento. Dois comportamentos viram dois testes.

## Resultados esperados e qualidade das verificações

Cada teste define uma entrada controlada, executa a operação e compara o resultado com uma expectativa explícita e independente da implementação testada. Um comportamento pode exigir várias verificações complementares no mesmo teste.

- **Sucesso:** confira valores relevantes, estado resultante e efeitos observáveis. Um status HTTP 200/201, `is_ok()` ou a existência de um objeto não bastam quando o contrato inclui conteúdo ou efeitos.
- **Erro:** confira a causa específica, o código e o contexto esperado. Verifique também efeitos que não podem ocorrer, como sobrescrever um arquivo sem confirmação ou publicar um evento de aceitação após recusar a entrada.
- **Expectativa independente:** use valores explícitos ou fixtures revisadas. Não calcule o resultado esperado pela própria função testada nem pela mesma lógica de produção que determina a propriedade em verificação.
- **JSON:** compare estruturas e valores esperados, incluindo a ausência de campos proibidos. Não dependa de indentação ou ordem das chaves. Conferir apenas a presença de um nome de campo não verifica seu valor.
- **Arquivos:** confira conteúdo, caminho e preservação ou backup dos dados anteriores, conforme o contrato. Use conteúdo esperado independente para validar o formato gerado.
- **Mensagens:** confira o texto exato quando a mensagem fizer parte do contrato do projeto. Mensagens de bibliotecas ou do sistema operacional podem variar; nesses casos, confira a causa tipada e o contexto, sem fixar a redação externa.
- **Campos variáveis:** valide propriedades concretas, como horário dentro do intervalo da operação, porta atribuída válida e caminho de backup existente com o conteúdo correto. Não ignore esses campos sem verificar suas garantias.
- **Assíncrono:** limite a espera por eventos e confira sua ordem e conteúdo quando fizerem parte do contrato. A ausência de eventos esperados deve produzir uma falha de teste, sem espera indefinida.

Exemplo de contrato GSI: ao receber um payload com `auth.token` e `map.game_state` igual a `X`, o teste deve verificar HTTP 200, evento de aceitação, JSON igual a `{"map":{"game_state":"X"}}` e classificação correta como evento do jogo. Um token inválido deve produzir HTTP 401 e rejeição por token inválido, sem publicar aceitação.

Para os erros padronizados, siga também [ERRORS.md](ERRORS.md#testes-do-padrão-de-erros): valide os campos obrigatórios, a causa conhecida ou `UNCLASSIFIED_ERROR`, a preservação da origem e a remoção de dados sensíveis. Cada falha investigada recebe teste de regressão.

### Exemplos de código ruim e bom

Os exemplos abaixo são ilustrativos e independentes das funções do projeto. Usam `serde_json` para mostrar a diferença entre aceitar qualquer resultado e conferir um contrato explícito. No código real, aplique as mesmas verificações à saída da operação testada.

**Ruim: sucesso sem conferir o conteúdo.** Este teste passa mesmo que o valor de `game_state` esteja errado ou que campos obrigatórios estejam ausentes.

```rust
#[test]
fn parses_game_state_without_checking_values() {
    let result = serde_json::from_str::<serde_json::Value>(
        r#"{"map":{"game_state":"PLAYING"},"is_test":false}"#,
    );

    assert!(result.is_ok());
}
```

**Bom: sucesso com resultado esperado explícito.** A comparação verifica a estrutura completa, os valores e a ausência de campos extras, sem depender da formatação do JSON.

```rust
#[test]
fn parses_expected_game_state_and_test_flag() {
    let payload = serde_json::from_str::<serde_json::Value>(
        r#"{"map":{"game_state":"PLAYING"},"is_test":false}"#,
    )
    .expect("a entrada do teste deve ser um JSON válido");

    let expected = serde_json::json!({
        "map": { "game_state": "PLAYING" },
        "is_test": false,
    });

    assert_eq!(payload, expected);
}
```

**Ruim: erro sem identificar a causa.** Qualquer falha faria o teste passar, inclusive uma diferente da esperada.

```rust
#[test]
fn rejects_json_without_checking_error_cause() {
    let result = serde_json::from_str::<serde_json::Value>("{]");

    assert!(result.is_err());
}
```

**Bom: erro com causa e contexto definidos.** O teste verifica a categoria e a localização da falha, sem depender da mensagem externa da biblioteca.

```rust
#[test]
fn rejects_invalid_json_with_syntax_error_at_expected_position() {
    let parse_error = serde_json::from_str::<serde_json::Value>("{]")
        .expect_err("a entrada malformada deve ser recusada");

    assert_eq!(parse_error.classify(), serde_json::error::Category::Syntax);
    assert_eq!(parse_error.line(), 1);
    assert_eq!(parse_error.column(), 2);
}
```

Esses exemplos verificam somente a conversão JSON. Um teste da aplicação também precisa verificar os efeitos do seu contrato: evento emitido, estado publicado, conteúdo gravado ou preservação de dados após uma falha. Testar apenas a biblioteca não substitui testar o comportamento da aplicação.

## Nomes dos testes

O nome do teste diz o que está sendo verificado, para que a saída do `cargo test` explique sozinha o que passou ou falhou. Ele é uma frase em inglês, em `snake_case`, com o comportamento esperado e a condição.

| Errado           | Certo                                                           |
| ---------------- | --------------------------------------------------------------- |
| `test1`          | `rejects_request_with_wrong_token`                              |
| `it_works`       | `creates_config_when_none_exists`                               |
| `reports_errors` | `fails_with_line_when_quote_is_not_closed`                      |
| `test_install`   | `asks_confirmation_and_keeps_file_when_existing_config_differs` |

- Formatos comuns: `<faz_algo>_when_<condição>`, `fails_when_<condição>`, `rejects_<entrada>_when_<condição>`.
- Funções auxiliares, fakes, constantes e variáveis dos testes seguem as mesmas regras de nomes do código ([CODE_STANDARDS.md](CODE_STANDARDS.md#nomes)). Exemplos: `fake_proc`, `SingleInstallLocator`, `temp_dir`, `config_path`.
- Um fake com comportamento não óbvio tem `///` dizendo o que ele simula.

## Organização

A implementação e os testes ficam em arquivos distintos.

| Tipo                         | Local definido                                                    | Como                                                                                                     |
| ---------------------------- | ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Unitário                     | `tests/unit/<caminho do módulo>/<nome>_tests.rs`                  | Espelha o caminho a partir de `src/`, preservando o acesso aos itens privados como módulo de teste filho |
| Casos de uso                 | `tests/unit/application/use_cases/`                               | Ports substituídos por fakes em memória                                                                  |
| Adaptadores isolados         | `tests/unit/gsi/` e `tests/unit/infrastructure/`                  | Diretórios temporários, `/proc` falso e servidores locais conforme o comportamento                       |
| Integração entre componentes | `tests/integration/`, registrado por `tests/integration_tests.rs` | Verifica contratos e efeitos do fluxo entre componentes                                                  |
| Ponta a ponta (E2E)          | `tests/e2e_tests.rs`                                              | Exercita o aplicativo completo por suas interfaces externas                                              |
| Auxiliares compartilhados    | `tests/support/`                                                  | Fakes e preparação, importados somente pelos testes                                                      |
| Dados de exemplo             | `tests/fixtures/`                                                 | Arquivos anonimizados e resultados esperados revisados                                                   |

Por exemplo, `src/domain/keyvalues.rs` corresponde a `tests/unit/domain/keyvalues_tests.rs`.

Regras de ligação e compilação:

- O arquivo de produção mantém somente a declaração do módulo de testes com `#[cfg(test)]` e `#[path = "..."]`, apontando para o arquivo espelhado. O caminho é relativo à pasta do arquivo que declara o módulo e precisa ser um texto literal: o `#[path]` não aceita macros como `env!`.
- Fixtures são lidas a partir da raiz do projeto, com `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/..."))`, para não depender da pasta do teste.
- Os eventos do GSI em `tests/fixtures/gsi/` vêm de partidas reais anonimizadas, mais uma fixture sintética de fim de partida identificada como tal. O README da pasta descreve a origem de cada arquivo; nenhuma fixture pode ter dados pessoais. Os testes unitários as leem por `tests/support` (`fixture_payloads`), e os de integração pelo próprio arquivo.
- Testes de sequência de eventos (`tests/integration/match_flow.rs`) comparam a saída com uma lista escrita à mão a partir da leitura da fixture, nunca gerada pelo próprio código testado.
- O arquivo espelhado é um módulo filho e pode acessar itens privados do pai por `super`. Não torne funções públicas apenas para testá-las.
- Corpos de testes, fakes e auxiliares ficam fora de `src/` e não entram no binário de produção.
- O Cargo descobre arquivos `.rs` diretamente na raiz de `tests/` como executáveis independentes. Os arquivos nas subpastas precisam ser declarados pelos respectivos módulos ou pontos de entrada; movê-los sem registrar não executa os testes.
- Testes de integração importam a aplicação pela biblioteca em `src/lib.rs` (`dota_companion::...`), que declara as camadas usadas por `main.rs`.
- Os auxiliares da integração ficam em `tests/integration/harness.rs`, porque importam `dota_companion::`. Os de `tests/support/` usam `crate::` e servem só aos unitários.
- Para verificar logs, use `capture_logs()` de `tests/support/`. Ele captura o `tracing` da thread do teste, incluindo as tarefas de um `#[tokio::test]`; código em `spawn_blocking` roda em outra thread e fica de fora. Todo teste de falha que registra log confere que segredos simulados não aparecem.
- Testes não leem o `.env`, o `config.toml` nem as variáveis reais da máquina: use arquivos em diretório temporário e uma busca de variáveis controlada.
- Integração não é sinônimo de E2E: testar dois componentes conectados não exige iniciar o aplicativo completo.

Por camada:

- **domain**: funções puras, entrada e saída. Inclua entradas malformadas.
- **application**: política dos casos de uso (ex.: não sobrescrever sem confirmação), sem acesso a disco ou rede.
- **gsi**: o router é testado direto com `tower::ServiceExt::oneshot`, sem abrir porta. O bind e o cliente usam servidor real em `127.0.0.1:0`.
- **infrastructure**: arquivos reais em diretório temporário, incluindo erros de permissão.
- **presentation**: regras que não dependem de widgets (formatação, traduções) têm teste unitário.
- **pânicos**: subprocesso isolado instala o hook, provoca uma falha com segredo no payload e no nome da thread e verifica a saída completa, localização e status de falha. O hook global do processo principal dos testes não é alterado.
- **ferramenta de envio**: o exemplo compilado pelo Cargo roda em subprocesso com ambiente e arquivos temporários. O servidor local verifica evento sintético único; falhas de token e de conexão verificam código de saída, diagnóstico e ausência do token no terminal.
- **integração**: núcleo real com o servidor GSI em porta livre e arquivos em diretório temporário. Toda espera por uma mudança no `AppSnapshot` tem tempo limite.

## Comandos

Todos precisam passar antes de cada commit. O script `scripts/check.sh` roda todos em sequência e para no primeiro erro:

| Comando                                     | Verifica                                              |
| ------------------------------------------- | ----------------------------------------------------- |
| `cargo fmt --check`                         | Formatação do código Rust (rustfmt)                   |
| `cargo clippy --all-targets -- -D warnings` | Lint do código Rust; qualquer aviso falha             |
| `cargo test`                                | Testes automatizados                                  |
| `dprint check`                              | Formatação dos arquivos Markdown e TOML               |
| `rumdl check .`                             | Lint de estrutura dos arquivos Markdown               |
| `typos`                                     | Ortografia de identificadores e comentários do código |

```sh
scripts/check.sh
```

Para corrigir formatação automaticamente: `cargo fmt`, `dprint fmt` e `rumdl fmt .`.

Para rodar um teste ou um módulo específico:

```sh
cargo test gsi::server
cargo test rejects_request_when_body_is_not_json
```
