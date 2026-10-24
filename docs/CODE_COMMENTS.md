# Padrão de comentários

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-24

Como e onde comentar o código.

## Objetivo

O comentário diz **o que** o código faz, para quem bate o olho entender rápido. Deve ser direto e objetivo. Os comentários são escritos em português (PT-BR).

## Onde comentar (obrigatório)

| Local                                                                                           | Forma        | Conteúdo                                                                                         |
| ----------------------------------------------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------------------------ |
| Topo de cada módulo                                                                             | `//!`        | O que o módulo faz e qual é o seu papel na arquitetura                                           |
| Itens públicos (`pub fn`, `pub struct`, `pub enum`, `pub trait`, variantes e campos não óbvios) | `///`        | O que o item faz ou representa, e as garantias importantes (ex.: "nunca sobrescreve sem backup") |
| Bloco `unsafe`                                                                                  | `// SAFETY:` | Por que a chamada é segura naquele ponto                                                         |

## Onde comentar (quando necessário)

Use comentários de linha (`//`) só para o que o código não deixa claro sozinho:

- uma regra de negócio ("heartbeat do Dota é 30 s; acima de 90 s a conexão não é considerada ativa");
- uma limitação do Dota, da Steam ou do Linux;
- um valor que parece arbitrário mas tem motivo.

## Proibido

- Histórico de conversa ou de desenvolvimento ("como pedido", "agora usamos", "antes era X").
- Opinião ("solução feia", "acho melhor").
- Repetir o código em texto (`// incrementa o contador` acima de `count += 1`).
- Planos ou referências ao futuro ("vai ser usado na próxima fase").
- Código comentado. Remova-o; o histórico está no git.
- `TODO` sem descrição clara do que falta.

## Exemplos

Certo:

```rust
//! Lê as bibliotecas Steam a partir de `libraryfolders.vdf`.

/// Grava em arquivo temporário e renomeia, para nunca deixar o arquivo pela metade.
pub fn write(...)

// A Valve compara chaves sem diferenciar maiúsculas.
```

Errado:

```rust
// Função que escreve o arquivo        ← repete o nome da função
// Mudei para rename porque estava falhando   ← histórico
// Isso aqui ficou meio confuso           ← opinião
// Será usado depois pelo histórico      ← futuro
```

## Manutenção

Ao mudar o comportamento de um trecho, atualize o comentário no mesmo commit. Um comentário desatualizado é pior que nenhum.
