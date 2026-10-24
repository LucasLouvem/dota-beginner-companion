# Padrão de código

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-24

Regras para manter o código formatado, verificado, organizado, com nomes claros, baseado na documentação e sem duplicação.

## Formatação e verificação

Todo código Rust passa pelo formatador e pelo verificador antes de cada commit:

| Ferramenta | Comando                                     | Regra                                               |
| ---------- | ------------------------------------------- | --------------------------------------------------- |
| rustfmt    | `cargo fmt`                                 | Configuração padrão; o código não é formatado à mão |
| clippy     | `cargo clippy --all-targets -- -D warnings` | Nenhum aviso é aceito                               |

- Para corrigir a formatação, rode `cargo fmt`. Não ajuste espaçamento ou quebras de linha manualmente.
- Corrija o aviso do clippy em vez de silenciá-lo. Um `#[allow(...)]` só é aceito com um comentário explicando o motivo.
- O `scripts/check.sh` roda essas e as demais verificações ([TESTING.md](TESTING.md#comandos)).

## Organização

- Cada função faz uma coisa. Se o nome precisa de "e", provavelmente são duas funções.
- Prefira retorno antecipado (`let ... else`, `?`) a `if` aninhados.
- Valores com significado viram constantes nomeadas (ex.: porta padrão, limites, intervalos).
- Use tipos (enums, newtypes) para representar estados, em vez de combinações de booleanos ou strings.
- O código fica na camada correta, conforme [ARCHITECTURE.md](ARCHITECTURE.md).
- Identificadores em inglês. Comentários seguem [CODE_COMMENTS.md](CODE_COMMENTS.md).

## Nomes

Todo nome diz o que a coisa é ou faz, sem precisar ler o resto do código. As mesmas regras valem para os testes ([TESTING.md](TESTING.md#nomes-dos-testes)). Nomes de uma letra ou genéricos (`x`, `s`, `e`, `data`, `tmp`, `aux`, `foo`) são proibidos em variáveis, parâmetros, campos, funções e closures.

Convenções do Rust:

| Item                                         | Estilo                 | Exemplo                       |
| -------------------------------------------- | ---------------------- | ----------------------------- |
| Variáveis, funções, métodos, módulos, campos | `snake_case`           | `config_path`, `load_or_init` |
| Tipos, traits, variantes de enum             | `UpperCamelCase`       | `GsiEndpoint`, `ServerState`  |
| Constantes e statics                         | `SCREAMING_SNAKE_CASE` | `DEFAULT_GSI_PORT`            |
| Parâmetros genéricos de tipo e lifetimes     | letra maiúscula / `'a` | `T`, `E`, `'a`                |

Regras:

- Funções começam com verbo (`render`, `install_gsi_config`, `send_test_event`).
- Booleanos são perguntas: `is_test`, `has_token`, `dota_running`.
- Conversões seguem o padrão da linguagem: `as_` (barata, por referência), `to_` (cria um valor novo), `into_` (consome o valor).
- Sem abreviações próprias. Só as siglas e abreviações conhecidas da área e da biblioteca padrão: `id`, `uri`, `url`, `http`, `json`, `gsi`, `cfg`, `io`, `dir`, `secs`, `msgid` (gettext).
- Erros capturados têm nome do que representam (`Err(bind_error)`, `Err(io_error)`), não `e`.
- Unidades no nome quando o tipo não deixa claro (`timeout_secs`, `max_body_bytes`).
- Generics (`T`, `E`) e lifetimes (`'a`) são as únicas exceções de uma letra.

## Documentação antes de implementar

Nenhuma implementação começa por suposição. A ordem de consulta é:

1. **Documentação do projeto** (`docs/`) e o código existente.
2. **Documentação oficial** da ferramenta, crate, API ou módulo usado: docs.rs, guias e exemplos do próprio projeto, documentação da Valve/Steam. Sempre na versão declarada no `Cargo.toml`.
3. **Solução própria**, só quando nenhuma das duas cobre o caso. Nesse caso, um comentário explica a limitação que levou a ela ([CODE_COMMENTS.md](CODE_COMMENTS.md)).

Regras:

- Use a API como a documentação define. Não invente funções, parâmetros, formatos ou comportamentos.
- Quando a documentação oferece uma forma recomendada (ex.: padrões do Relm4, extratores do Axum), use essa forma em vez de uma alternativa própria.
- Comportamento observado e não documentado (ex.: campos do payload GSI) é tratado como incerto: validado em teste com dados reais e nunca apresentado como garantido.

## Reaproveitamento antes de criar

Antes de escrever uma função, tipo, widget ou teste auxiliar novo, procure se já existe algo que resolve o problema:

1. Busque no `src/` por nomes e comportamentos parecidos (`grep`, busca do editor).
2. Verifique os pontos de reuso do projeto:
   - **domain**: tipos e regras do Dota e da Steam, como o parser KeyValues e o modelo de configuração GSI;
   - **application/ports**: acesso ao sistema já abstraído (Steam, processos, arquivos, conexão GSI);
   - **presentation/widgets**: linhas de status, títulos de seção, listas e formatação de horário;
   - **presentation/i18n**: tradução e substituição de valores em textos;
   - **test_support** dos casos de uso: fakes de ports para testes.
3. Se algo parecido existe, use-o ou estenda-o. Não crie uma segunda versão.
4. Se a mesma lógica aparece em dois lugares, extraia para um ponto único no mesmo commit.
5. Não crie abstrações para usos hipotéticos. Extraia quando houver pelo menos dois usos reais.

Na revisão, código duplicado é tratado como defeito.
