# Erros

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-24

Padrão para definir e exibir erros, e catálogo dos erros tratados pelo aplicativo.

## Padrão

- Cada módulo define o seu próprio tipo de erro: um `enum` com `thiserror`, com nome terminado em `Error` (ex.: `KvError`).
- Cada variante representa **uma causa específica** e carrega o contexto necessário: caminho, porta, linha, causa original.
- A mensagem (`#[error("...")]`) diz o que falhou e onde, em PT-BR (ex.: "texto entre aspas não terminado na linha 3").
- Proibido:
  - erro genérico como tipo de retorno (`String`, `Box<dyn Error>`);
  - `anyhow` fora de `main.rs`;
  - mensagens vagas ("erro", "falhou", "algo deu errado");
  - `unwrap()`/`expect()` em código de produção, exceto quando a falha for impossível. Nesse caso, a mensagem do `expect` diz por quê.
- Erros nunca contêm o token do GSI nem outros dados sensíveis.
- Resultados esperados não são erros. Um arquivo ausente ou que precisa de confirmação vira variante de estado (ex.: `GsiConfigState::Missing`), não `Err`.
- A interface traduz cada erro em um título e uma **ação** para o usuário, nas linhas de status do Diagnóstico.

## Catálogo

### Servidor GSI

| Situação                                | Resposta / estado                                         | Ação para o usuário                           |
| --------------------------------------- | --------------------------------------------------------- | --------------------------------------------- |
| Porta em uso ou sem permissão para bind | Estado "Servidor local não iniciou", com endereço e causa | Alterar `gsi_port` e reinstalar o arquivo GSI |
| Token ausente ou diferente              | HTTP 401; o contador de recusadas aumenta                 | Reinstalar o arquivo GSI                      |
| Corpo que não é JSON válido             | HTTP 400; o contador de recusadas aumenta                 | Verificar quem está enviando para a porta     |
| Corpo acima de 1 MB                     | HTTP 413                                                  | —                                             |

### Arquivo de configuração GSI

| Situação                                       | Estado                                                 | Ação para o usuário                                                  |
| ---------------------------------------------- | ------------------------------------------------------ | -------------------------------------------------------------------- |
| Arquivo não existe                             | "Arquivo GSI não instalado"                            | Clicar em "Instalar configuração GSI"                                |
| `uri` aponta para outra porta/endereço         | "Arquivo GSI aponta para outro endereço"               | Reinstalar                                                           |
| Token diferente do `config.toml`               | "Token do arquivo GSI diferente do aplicativo"         | Reinstalar                                                           |
| Conteúdo não é KeyValues válido                | "Arquivo GSI inválido", com a causa do parser          | Reinstalar (com backup)                                              |
| Sem permissão de leitura                       | "Não foi possível ler o arquivo GSI"                   | Corrigir as permissões da pasta do Dota                              |
| Arquivo existente diferente na instalação      | "Instalação aguardando confirmação" + diálogo          | Confirmar a substituição; o backup `.bak-<timestamp>` é criado antes |
| Falha ao criar a pasta, gravar ou fazer backup | "Falha ao instalar o arquivo GSI", com caminho e causa | Corrigir as permissões da pasta `game/dota/cfg`                      |

### Formato KeyValues (`KvError`)

| Variante                    | Significado                       |
| --------------------------- | --------------------------------- |
| `UnterminatedString(linha)` | Aspas abertas e não fechadas      |
| `UnexpectedClose(linha)`    | `}` sem seção aberta              |
| `MissingValue(linha)`       | Chave sem valor ou `{` sem chave  |
| `UnclosedSection`           | Seção aberta até o fim do arquivo |

### Steam e Dota

| Situação                                   | Estado                                                              | Ação para o usuário                                                             |
| ------------------------------------------ | ------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Nenhuma instalação do Dota encontrada      | "Dota 2 não encontrado", com as bibliotecas verificadas             | Adicionar a biblioteca em `extra_libraries` ou `DOTA_COMPANION_EXTRA_LIBRARIES` |
| `libraryfolders.vdf` ausente ou malformado | O arquivo é ignorado e as outras fontes continuam sendo verificadas | —                                                                               |

### Configuração do aplicativo

| Situação                            | Comportamento                                                 | Ação para o usuário                                      |
| ----------------------------------- | ------------------------------------------------------------- | -------------------------------------------------------- |
| `config.toml` com conteúdo inválido | O aplicativo encerra e mostra o caminho e a causa no terminal | Corrigir ou apagar o arquivo (um novo token será gerado) |

### Teste de conexão

| Situação                                           | Estado                                       | Ação para o usuário                           |
| -------------------------------------------------- | -------------------------------------------- | --------------------------------------------- |
| Servidor não respondeu                             | "Teste de conexão falhou", com a causa       | Verificar o estado do servidor no Diagnóstico |
| Servidor respondeu com status diferente de sucesso | "Teste de conexão falhou", com o status HTTP | Verificar porta e token                       |
