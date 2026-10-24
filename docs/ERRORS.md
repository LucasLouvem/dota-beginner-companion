# Erros

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-24

Padrão para definir e exibir erros, e catálogo dos erros tratados pelo aplicativo.

## Padrão

- Existe um único contrato de diagnóstico para todos os erros apresentados ao usuário, definido na seção "Contrato central". Os módulos não criam formatos alternativos.
- Cada módulo representa suas causas com um `enum` tipado usando `thiserror`, com nome terminado em `Error` (ex.: `KeyValuesError`). Esses tipos seguem o mesmo contrato; não são padrões independentes.
- Cada variante representa **uma causa específica** e carrega o contexto necessário: caminho, porta, linha, causa original.
- A mensagem (`#[error("...")]`) diz o que falhou e onde, em PT-BR (ex.: "texto entre aspas não terminado na linha 3").
- Proibido:
  - erro genérico como tipo de retorno (`String`, `Box<dyn Error>`);
  - `anyhow` fora de `main.rs`;
  - mensagens vagas ("erro", "falhou", "algo deu errado");
  - `unwrap()`/`expect()` em código de produção, exceto quando a falha for impossível. Nesse caso, a mensagem do `expect` diz por quê.
- Erros nunca contêm o token do GSI nem outros dados sensíveis.
- Resultados esperados não são erros. Um arquivo ausente ou que precisa de confirmação vira variante de estado (ex.: `GsiConfigState::Missing`), não `Err`.
- A interface renderiza o diagnóstico padronizado nas linhas de status do Diagnóstico. A classificação da causa e a orientação ficam fora dos widgets.

## Contrato central

Esta seção define o padrão obrigatório para a adequação do código. A centralização ainda não está implementada; o catálogo abaixo descreve os comportamentos atuais.

O contrato e a estrutura do diagnóstico devem ser definidos em um único arquivo, `application/error.rs`. Em Rust, o equivalente a herdar uma estrutura e especializar seu conteúdo é usar composição e traits. Erros específicos preservam seus tipos e causas, mas sua conversão para diagnóstico segue a mesma estrutura central.

| Campo               | Obrigatório                  | Regra                                                                                                                      |
| ------------------- | ---------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `code`              | Sim                          | Identificador estável em inglês, em `SCREAMING_SNAKE_CASE`, como `GSI_BIND_FAILED`. Não muda quando o texto é revisado.    |
| `title`             | Sim                          | Título curto em PT-BR que identifica a falha.                                                                              |
| `cause`             | Sim                          | Explicação em PT-BR do que falhou, com o contexto disponível. Não afirma uma causa que não foi identificada.               |
| `resolution`        | Sim                          | Instrução simples em PT-BR para resolver ou investigar a falha. Não promete uma solução desconhecida.                      |
| `technical_details` | Quando houver erro de origem | Detalhes do erro original e sua cadeia de causas, após remoção de dados sensíveis. Não substituem a explicação ao usuário. |

Regras de organização:

- O domínio mantém erros puros e não depende de `application`. A aplicação converte esses erros para o contrato central.
- Adaptadores preservam os erros de origem usando `source` ou `#[from]`, conforme o caso. A conversão não pode descartar a causa original nem reduzi-la antecipadamente a `String`.
- O contrato central padroniza o diagnóstico; não exige um enum global com todas as falhas de todas as camadas.
- Novas classificações recebem códigos únicos e entram neste documento com sua causa e orientação. Nenhum módulo cria uma segunda estrutura de diagnóstico.
- Campos obrigatórios não podem estar vazios. Códigos não dependem de mensagens de bibliotecas, do idioma ou de valores variáveis como caminhos e portas.
- Causas são classificadas por variantes e códigos tipados das bibliotecas, quando disponíveis. Não compare trechos de mensagens para decidir o tratamento.
- Códigos HTTP pertencem somente ao adaptador HTTP do GSI. Erros de arquivo, domínio ou interface não recebem um `statuscode` artificial.
- A cadeia técnica preservada internamente pode conter contexto sensível; somente sua versão sanitizada pode aparecer na interface, no terminal ou nos logs. Não registre tokens, payloads completos ou dados pessoais para diagnosticar erros.

### Exemplo de diagnóstico conhecido

Exemplo ilustrativo do contrato, sem exigir serialização JSON no aplicativo:

```text
code: GSI_BIND_FAILED
title: Servidor GSI não iniciou
cause: A porta 53000 já está sendo usada.
resolution: Escolha outra porta na configuração e reinstale o arquivo GSI.
technical_details: Não foi possível escutar em 127.0.0.1:53000: endereço em uso.
```

Essa orientação só se aplica quando o erro original identifica porta em uso. Outras causas de falha ao iniciar o servidor precisam de classificação própria ou do tratamento abaixo.

## Causas ainda não classificadas

Uma falha sem correspondência nas causas conhecidas deve continuar sendo propagada como erro, preservando a origem e o contexto da operação. Ela não vira sucesso, não é ignorada e não recebe uma causa conhecida por aproximação.

Na fronteira em que o erro é apresentado ou registrado, use o mesmo contrato central com o código reservado `UNCLASSIFIED_ERROR`:

- `title` identifica a operação que não pôde ser concluída.
- `cause` informa o contexto conhecido e deixa claro que a causa ainda não foi classificada.
- `resolution` orienta a consultar os detalhes técnicos para investigação, sem inventar uma correção.
- `technical_details` mantém a mensagem técnica original disponível e sua cadeia de causas, com dados sensíveis removidos.

Preservar o erro original significa propagá-lo com `Result` e disponibilizar seus detalhes. Não significa provocar `panic!`, usar `unwrap()` ou encerrar o aplicativo indiscriminadamente. Uma operação recuperável informa a falha e permite continuar; uma falha que impede a inicialização encerra o aplicativo com contexto no terminal.

O tratamento de causas desconhecidas é obrigatório, mas não substitui classificações já conhecidas. Depois de investigar uma ocorrência:

1. Identifique a causa a partir do erro original e do contexto.
2. Acrescente a variante ou o mapeamento específico, com código estável, explicação e orientação adequada.
3. Atualize o catálogo deste documento.
4. Adicione um teste de regressão que reproduza a falha e confirme sua classificação.

## Testes do padrão de erros

Além das regras de [TESTING.md](TESTING.md), os testes devem verificar:

- **Causa conhecida:** a falha recebe o código esperado, contexto correto e diagnóstico com todos os campos obrigatórios preenchidos.
- **Causa desconhecida:** o diagnóstico recebe `UNCLASSIFIED_ERROR`, preserva os detalhes disponíveis e não se passa por uma causa conhecida.
- **Preservação:** a conversão mantém o erro original e sua cadeia de causas acessíveis; não guarda apenas uma mensagem genérica.
- **Dados sensíveis:** interface, terminal e logs não expõem segredos presentes em erros de origem simulados.
- **Regressão:** cada causa investigada passa a ter um teste que reproduz a condição e verifica o tratamento específico.

Prefira conferir código, contexto e conteúdo necessário à orientação. Não compare frases inteiras quando a redação exata não fizer parte do comportamento exigido. Os testes devem provocar a condição de falha com entradas controladas ou fakes, sem depender de erros reais da máquina do desenvolvedor.

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

### Formato KeyValues (`KeyValuesError`)

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

### Log de eventos GSI (`GsiRecordError`)

| Variante         | Estado                                                       | Ação para o usuário                                                   |
| ---------------- | ------------------------------------------------------------ | --------------------------------------------------------------------- |
| `Write(tipo)`    | "Falha ao gravar o log de eventos GSI", com caminho e motivo | Criar a pasta do arquivo ou corrigir a permissão; o app segue rodando |
| `InvalidPayload` | Mesma linha, com "o evento recebido não é um JSON válido"    | —                                                                     |

### Teste de conexão

| Situação                                                                                     | Estado                                            | Ação para o usuário                           |
| -------------------------------------------------------------------------------------------- | ------------------------------------------------- | --------------------------------------------- |
| Servidor não respondeu                                                                       | "Teste de conexão falhou", com a causa            | Verificar o estado do servidor no Diagnóstico |
| Servidor respondeu com status diferente de sucesso                                           | "Teste de conexão falhou", com o status HTTP      | Verificar porta e token                       |
| Cliente HTTP não pôde ser criado na inicialização (`HttpConnectionTesterError::ClientBuild`) | O aplicativo encerra e mostra a causa no terminal | Consultar a causa exibida no terminal         |
