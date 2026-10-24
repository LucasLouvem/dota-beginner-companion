# Testes automatizados

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-24

Regras e organização dos testes do projeto.

## Regras

- **Testes são obrigatórios.** Todo comportamento novo ou alterado vem com testes no mesmo commit.
- Cada comportamento tem **caso de sucesso e caso de erro**. Exemplos: token válido e token inválido; arquivo criado e falha de permissão; arquivo VDF válido e malformado.
- Os testes não dependem de rede externa, do Dota instalado, da Steam do desenvolvedor nem da ordem de execução.
- Os testes não alteram arquivos fora de diretórios temporários.
- Um teste verifica um comportamento. O nome diz qual é esse comportamento e a condição (`rejects_wrong_or_missing_token`, `asks_before_replacing_and_backs_up`).

## Organização

| Tipo                     | Onde                                                    | Como                                                                          |
| ------------------------ | ------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Unitário                 | Módulo `#[cfg(test)] mod tests` no mesmo arquivo        | Testa funções e tipos daquele módulo                                          |
| Casos de uso             | `application/use_cases`, com os fakes de `test_support` | Ports substituídos por implementações em memória                              |
| Adaptadores              | No próprio módulo do adaptador                          | Diretório temporário (`tempfile`), `/proc` falso, servidor local na porta `0` |
| Integração entre camadas | `tests/`                                                | Fluxo completo usando fakes dos ports                                         |
| Dados de exemplo         | `tests/fixtures/`                                       | Arquivos reais anonimizados (VDF, payloads GSI)                               |

Por camada:

- **domain**: funções puras, entrada e saída. Inclua entradas malformadas.
- **application**: política dos casos de uso (ex.: não sobrescrever sem confirmação), sem acesso a disco ou rede.
- **gsi**: o router é testado direto com `tower::ServiceExt::oneshot`, sem abrir porta. O bind e o cliente usam servidor real em `127.0.0.1:0`.
- **infrastructure**: arquivos reais em diretório temporário, incluindo erros de permissão.
- **presentation**: regras que não dependem de widgets (formatação, traduções) têm teste unitário.

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
cargo test rejects_invalid_json
```
