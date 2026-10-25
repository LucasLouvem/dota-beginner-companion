# Erros

**Criado em:** 2026-10-24 · **Atualizado em:** 2026-10-25

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

O contrato e a estrutura do diagnóstico ficam em um único arquivo, `application/error.rs`. Em Rust, o equivalente a herdar uma estrutura e especializar seu conteúdo é usar composição e traits. Erros específicos preservam seus tipos e causas, mas sua conversão para diagnóstico segue a mesma estrutura central.

- `Diagnostic` reúne os campos da tabela abaixo; `DiagnosticCode` lista os códigos do catálogo.
- Cada erro específico implementa a trait `ToDiagnostic`. Causas desconhecidas usam `Diagnostic::unclassified`.
- Os textos são msgids em PT-BR (`UiText`), traduzidos pela apresentação.
- `technical_details` é a mensagem do erro seguida das mensagens de cada causa (`source`), com o token trocado por `[removido]` pelo `Redactor`.

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
- Diagnósticos registrados em log usam `log_error` ou `log_warning` (`application/error.rs`): só o código e os detalhes técnicos sanitizados, com a mensagem em inglês.
- Canal fechado entre o servidor GSI, o núcleo e a interface é sempre falha inesperada com o aplicativo aberto: no encerramento normal o runtime descarta as tarefas antes que elas percebam o canal fechado. A falha é informada por outro caminho (log ou estado local da interface), nunca só pelo canal que falhou.
- Ignorar um resultado (`let _ =`) só é aceito com um comentário que explique por que ignorar é correto naquele ponto.

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

## Saída de pânicos

Pânicos representam falhas internas, não erros recuperáveis. O aplicativo e a ferramenta de envio sintético instalam um hook antes das outras operações. A saída vai diretamente para stderr, independente do filtro de log, com a mensagem "Falha interna (pânico)" e a localização (nome do arquivo, linha e coluna). Sem localização disponível, a mensagem informa essa ausência.

O hook não imprime o payload do pânico, nomes de threads, caminhos completos ou backtrace, mesmo com `RUST_BACKTRACE=full`. Não chama o hook padrão e não converte o pânico em sucesso: tarefas continuam produzindo `JoinError`, e pânicos não recuperados continuam encerrando a execução. Uma falha ao escrever em stderr é ignorada para não provocar um segundo pânico dentro do hook.

## Testes do padrão de erros

Além das regras de [TESTING.md](TESTING.md), os testes devem verificar:

- **Causa conhecida:** a falha recebe o código esperado, contexto correto e diagnóstico com todos os campos obrigatórios preenchidos.
- **Causa desconhecida:** o diagnóstico recebe `UNCLASSIFIED_ERROR`, preserva os detalhes disponíveis e não se passa por uma causa conhecida.
- **Preservação:** a conversão mantém o erro original e sua cadeia de causas acessíveis; não guarda apenas uma mensagem genérica.
- **Dados sensíveis:** interface, terminal e logs não expõem segredos presentes em erros de origem simulados.
- **Regressão:** cada causa investigada passa a ter um teste que reproduz a condição e verifica o tratamento específico.

Prefira conferir código, contexto e conteúdo necessário à orientação. Não compare frases inteiras quando a redação exata não fizer parte do comportamento exigido. Os testes devem provocar a condição de falha com entradas controladas ou fakes, sem depender de erros reais da máquina do desenvolvedor.

## Catálogo

A coluna "Código" mostra o `DiagnosticCode` exibido. Situações com "—" são estados ou respostas esperadas, não erros.

### Servidor GSI (`GsiServerError`)

| Situação                                | Código                           | Estado ou resposta                                   | Ação para o usuário                                           |
| --------------------------------------- | -------------------------------- | ---------------------------------------------------- | ------------------------------------------------------------- |
| Porta em uso                            | `GSI_BIND_FAILED`                | "Servidor GSI não iniciou", com a porta              | Escolher outra porta em `gsi_port` e reinstalar o arquivo GSI |
| Sem permissão para escutar na porta     | `GSI_BIND_PERMISSION_DENIED`     | "Servidor GSI não iniciou", com a porta              | Escolher uma porta acima de 1024 e reinstalar o arquivo GSI   |
| Outra falha ao escutar na porta         | `UNCLASSIFIED_ERROR`             | "Servidor GSI não iniciou", com os detalhes técnicos | Consultar os detalhes técnicos                                |
| O servidor parou de atender requisições | `UNCLASSIFIED_ERROR`             | "Servidor GSI parou", com os detalhes técnicos       | Consultar os detalhes técnicos                                |
| Token ausente ou diferente              | —                                | HTTP 401; o contador de recusadas aumenta            | Reinstalar o arquivo GSI                                      |
| Corpo que não é JSON válido             | —                                | HTTP 400; o contador de recusadas aumenta            | Verificar quem está enviando para a porta                     |
| Corpo acima de 1 MB                     | —                                | HTTP 413                                             | —                                                             |
| O núcleo não recebe o evento aceito     | `CORE_UNAVAILABLE`, só no log    | HTTP 503                                             | Reiniciar o aplicativo                                        |
| O núcleo não recebe uma recusa          | `CORE_UNAVAILABLE`, aviso no log | HTTP 401 ou 400; a recusa não é contada              | Reiniciar o aplicativo                                        |

### Arquivo de configuração GSI (`GsiConfigFileError`, `InstallError`)

| Situação                                                 | Código                                 | Estado                                                         | Ação para o usuário                                                  |
| -------------------------------------------------------- | -------------------------------------- | -------------------------------------------------------------- | -------------------------------------------------------------------- |
| Arquivo não existe                                       | —                                      | "Arquivo GSI não instalado"                                    | Clicar em "Instalar configuração GSI"                                |
| `uri` aponta para outra porta ou endereço                | —                                      | "Arquivo GSI aponta para outro endereço"                       | Reinstalar                                                           |
| Token diferente do `config.toml`                         | —                                      | "Token do arquivo GSI diferente do aplicativo"                 | Reinstalar                                                           |
| Conteúdo não é KeyValues válido ou não tem seção         | `GSI_CONFIG_INVALID`                   | "Arquivo GSI inválido", com a causa do parser nos detalhes     | Reinstalar (com backup)                                              |
| Sem permissão de leitura                                 | `GSI_CONFIG_READ_PERMISSION_DENIED`    | "Não foi possível ler o arquivo GSI"                           | Corrigir as permissões da pasta do Dota                              |
| Outra falha de leitura                                   | `UNCLASSIFIED_ERROR`                   | "Não foi possível ler o arquivo GSI", com os detalhes técnicos | Consultar os detalhes técnicos                                       |
| Arquivo existente diferente na instalação                | —                                      | "Instalação aguardando confirmação" + diálogo                  | Confirmar a substituição; o backup `.bak-<timestamp>` é criado antes |
| Sem permissão para criar a pasta, gravar ou fazer backup | `GSI_CONFIG_INSTALL_PERMISSION_DENIED` | "Falha ao instalar o arquivo GSI", com o caminho               | Corrigir as permissões da pasta `game/dota/cfg`                      |
| Outra falha ao criar a pasta, gravar ou fazer backup     | `UNCLASSIFIED_ERROR`                   | "Falha ao instalar o arquivo GSI", com os detalhes técnicos    | Consultar os detalhes técnicos                                       |

### Formato KeyValues (`KeyValuesError`, `GsiConfigParseError`)

Erros do domínio, sem código próprio. Em um `.cfg` do GSI, qualquer um deles vira `GSI_CONFIG_INVALID`.

| Variante                                    | Significado                       |
| ------------------------------------------- | --------------------------------- |
| `KeyValuesError::UnterminatedString(linha)` | Aspas abertas e não fechadas      |
| `KeyValuesError::UnexpectedClose(linha)`    | `}` sem seção aberta              |
| `KeyValuesError::MissingValue(linha)`       | Chave sem valor ou `{` sem chave  |
| `KeyValuesError::UnclosedSection`           | Seção aberta até o fim do arquivo |
| `GsiConfigParseError::MissingSection`       | O `.cfg` não tem nenhuma seção    |

### Steam e Dota

| Situação                                   | Estado                                                              | Ação para o usuário                                                             |
| ------------------------------------------ | ------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Nenhuma instalação do Dota encontrada      | "Dota 2 não encontrado", com as bibliotecas verificadas             | Adicionar a biblioteca em `extra_libraries` ou `DOTA_COMPANION_EXTRA_LIBRARIES` |
| `libraryfolders.vdf` ausente ou malformado | O arquivo é ignorado e as outras fontes continuam sendo verificadas | —                                                                               |

### Inicialização (`ConfigError`, `HttpConnectionTesterError`)

O aplicativo encerra e mostra o diagnóstico no terminal. Os detalhes de um `config.toml` inválido trazem só a linha e a coluna, porque a mensagem do parser cita o conteúdo do arquivo, que pode conter o token.

| Situação                                             | Código                         | Ação para o usuário                                                                |
| ---------------------------------------------------- | ------------------------------ | ---------------------------------------------------------------------------------- |
| `config.toml` com conteúdo inválido                  | `APP_CONFIG_INVALID`           | Corrigir ou apagar o arquivo (um novo token será gerado; reinstalar o arquivo GSI) |
| Sem permissão para ler ou gravar o `config.toml`     | `APP_CONFIG_PERMISSION_DENIED` | Corrigir as permissões do arquivo ou da pasta                                      |
| Outra falha ao ler, gravar ou gerar o token          | `UNCLASSIFIED_ERROR`           | Consultar os detalhes técnicos                                                     |
| Cliente HTTP do teste de conexão não pôde ser criado | `UNCLASSIFIED_ERROR`           | Consultar os detalhes técnicos                                                     |

### Arquivo `.env` (`EnvFileError`)

O `.env` é opcional e lido só no diretório de execução, sem alterar o ambiente do processo. Se ele falhar, o aplicativo encerra antes de usar qualquer valor do arquivo e mostra o diagnóstico no terminal. Os detalhes de uma linha inválida trazem só a linha e a coluna, porque a mensagem do dotenvy cita a linha inteira, que pode conter segredos.

| Situação                          | Código                       | Ação para o usuário                                 |
| --------------------------------- | ---------------------------- | --------------------------------------------------- |
| Arquivo ausente                   | —                            | —                                                   |
| Linha com sintaxe inválida        | `ENV_FILE_INVALID`           | Corrigir a linha indicada ou remover o `.env`       |
| Sem permissão de leitura          | `ENV_FILE_PERMISSION_DENIED` | Corrigir a permissão do arquivo ou remover o `.env` |
| Outra falha ao ler ou interpretar | `UNCLASSIFIED_ERROR`         | Consultar os detalhes técnicos                      |

### Avisos na inicialização (`LogFilterError`, `TranslationSetupError`)

O aplicativo continua com um fallback e registra o diagnóstico no log. As traduções geram um aviso, que só aparece com `RUST_LOG` em `warn` ou mais detalhado; o `RUST_LOG` inválido é registrado como erro, porque o filtro de fallback só mostra erros.

| Situação                             | Código                     | Fallback              | Ação para o usuário                            |
| ------------------------------------ | -------------------------- | --------------------- | ---------------------------------------------- |
| `RUST_LOG` com diretiva inválida     | `UNCLASSIFIED_ERROR`       | O log mostra só erros | Corrigir o `RUST_LOG`                          |
| Idioma do sistema não suportado      | `TRANSLATIONS_UNAVAILABLE` | Textos em português   | Verificar se o idioma de `LANG` está instalado |
| Traduções não puderam ser carregadas | `TRANSLATIONS_UNAVAILABLE` | Textos em português   | Verificar a instalação das traduções           |

### Núcleo e tarefas (`TaskInterrupted`, `CoreConnectionError`)

Os detalhes técnicos trazem só a operação e a causa: a mensagem de um pânico pode conter dados processados e não aparece.

| Situação                                                                         | Código                  | Estado                                                                   | Ação para o usuário                                |
| -------------------------------------------------------------------------------- | ----------------------- | ------------------------------------------------------------------------ | -------------------------------------------------- |
| Diagnóstico, instalação ou teste de conexão terminou com pânico ou foi cancelado | `CORE_TASK_INTERRUPTED` | Linha da operação interrompida; o teste sai de "Testando…"               | Tentar de novo; se repetir, reiniciar o aplicativo |
| A gravação do log de eventos parou                                               | `CORE_TASK_INTERRUPTED` | "A gravação do log de eventos GSI parou"; nenhum evento é gravado depois | Reiniciar o aplicativo                             |
| A tela deixou de receber o estado                                                | `CORE_UNAVAILABLE`      | "O núcleo do aplicativo parou"; a barra mostra "GSI: núcleo parado"      | Reiniciar o aplicativo                             |
| Um comando da tela não foi recebido                                              | `CORE_UNAVAILABLE`      | "O núcleo do aplicativo parou", com o comando                            | Reiniciar o aplicativo                             |

### Log de eventos GSI (`GsiRecordError`)

O aplicativo continua rodando; a linha "Falha ao gravar o log de eventos GSI" mostra o diagnóstico.

| Situação                      | Código                      | Ação para o usuário                               |
| ----------------------------- | --------------------------- | ------------------------------------------------- |
| A pasta do arquivo não existe | `GSI_LOG_DIRECTORY_MISSING` | Criar a pasta ou alterar `DOTA_COMPANION_GSI_LOG` |
| Sem permissão para gravar     | `GSI_LOG_PERMISSION_DENIED` | Corrigir a permissão do arquivo ou da pasta       |
| Outra falha ao gravar         | `UNCLASSIFIED_ERROR`        | Consultar os detalhes técnicos                    |
| O evento não é um JSON válido | `GSI_LOG_INVALID_PAYLOAD`   | —                                                 |

### Teste de conexão (`ConnectionTestError`)

Todas as falhas aparecem como "Teste de conexão falhou". O status HTTP é classificado só no adaptador `gsi::client` e aparece apenas nos detalhes técnicos.

| Situação                             | Código                             | Ação para o usuário                                             |
| ------------------------------------ | ---------------------------------- | --------------------------------------------------------------- |
| Conexão recusada                     | `CONNECTION_TEST_UNREACHABLE`      | Verificar no Diagnóstico se o servidor local está ouvindo       |
| Sem resposta em 3 s                  | `CONNECTION_TEST_TIMED_OUT`        | Verificar o servidor local e tentar de novo                     |
| Token recusado (HTTP 401)            | `CONNECTION_TEST_TOKEN_REJECTED`   | Verificar se `gsi_port` não está sendo usada por outro programa |
| Conteúdo recusado (HTTP 400 ou 413)  | `CONNECTION_TEST_PAYLOAD_REJECTED` | Verificar se `gsi_port` não está sendo usada por outro programa |
| Outro status HTTP ou falha ao enviar | `UNCLASSIFIED_ERROR`               | Consultar os detalhes técnicos                                  |

### Banco local (`StorageError`)

A abertura do banco (`storage::schema`) e o adaptador `storage::sqlite_cache` classificam a falha pelo código que o SQLite informa. Cada migração do schema roda numa transação: se ela falha, o banco fica na versão anterior e a abertura devolve a falha. Um valor do cache com o prazo vencido não é erro: ele volta marcado como antigo.

| Situação                                                                                  | Código                | Ação para o usuário                                   |
| ----------------------------------------------------------------------------------------- | --------------------- | ----------------------------------------------------- |
| Sem permissão para abrir ou gravar o banco (`CannotOpen`, `PermissionDenied`, `ReadOnly`) | `STORAGE_UNAVAILABLE` | Verificar a permissão da pasta de dados do aplicativo |
| Arquivo corrompido ou que não é SQLite (`DatabaseCorrupt`, `NotADatabase`)                | `STORAGE_CORRUPTED`   | Apagar o arquivo do banco; o aplicativo cria um novo  |
| Banco criado por uma versão mais nova do aplicativo                                       | `UNCLASSIFIED_ERROR`  | Atualizar o aplicativo ou apagar o banco              |
| Migração do schema que falha (o banco continua na versão anterior)                        | `UNCLASSIFIED_ERROR`  | Consultar os detalhes técnicos                        |
| Outra falha do SQLite                                                                     | `UNCLASSIFIED_ERROR`  | Consultar os detalhes técnicos                        |

### Orçamento de chamadas ao OpenDota (`BudgetError`)

| Situação                                                                                     | Código                       | Ação para o usuário                                                                       |
| -------------------------------------------------------------------------------------------- | ---------------------------- | ----------------------------------------------------------------------------------------- |
| O limite do aplicativo para o dia ou o mês acabou (`BudgetExhausted`, com o limite atingido) | `OPENDOTA_BUDGET_EXHAUSTED`  | Esperar a renovação do limite (dia e mês em UTC); os dados guardados continuam aparecendo |
| Falha do banco local ao reservar                                                             | Os códigos de `StorageError` | Os mesmos do banco local                                                                  |

### OpenDota (`ProviderError`, `HttpJsonClientError`)

O cliente `providers::http_json` classifica a falha pela categoria que o reqwest informa e pelo status HTTP, sem ler mensagens. O caminho da consulta vai nos detalhes técnicos; nenhuma chave ou token.

| Situação                                                                                                                           | Código                      | Ação para o usuário                                           |
| ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------- | ------------------------------------------------------------- |
| Sem conexão com o OpenDota (rede ou DNS)                                                                                           | `OPENDOTA_UNREACHABLE`      | Verificar a internet; os dados guardados continuam aparecendo |
| O OpenDota não respondeu dentro do tempo limite                                                                                    | `OPENDOTA_TIMED_OUT`        | Tentar de novo mais tarde                                     |
| HTTP 429 também na última tentativa                                                                                                | `OPENDOTA_RATE_LIMITED`     | Esperar alguns minutos                                        |
| Status inesperado, resposta acima do limite de tamanho, JSON inválido ou sem o formato esperado (por exemplo, nenhum patch válido) | `OPENDOTA_BAD_RESPONSE`     | Tentar de novo mais tarde; o formato da API pode ter mudado   |
| Orçamento do aplicativo esgotado antes de uma tentativa                                                                            | `OPENDOTA_BUDGET_EXHAUSTED` | Ver "Orçamento de chamadas ao OpenDota"                       |
| O cliente HTTP não pôde ser criado                                                                                                 | `UNCLASSIFIED_ERROR`        | Consultar os detalhes técnicos                                |

### Dados dos heróis (`HeroDataError`)

| Situação                                              | Código               | Comportamento                                                                   |
| ----------------------------------------------------- | -------------------- | ------------------------------------------------------------------------------- |
| O recorte embutido do dotaconstants não pode ser lido | `UNCLASSIFIED_ERROR` | Erro no log; a aba Herói mostra só o que o GSI informa                          |
| Herói que o recorte não tem                           | —                    | A aba mostra o nome interno enviado pelo GSI e avisa que não há dados embutidos |

### Dados dos itens (`ItemDataError`)

| Situação                                              | Código               | Comportamento                                                                               |
| ----------------------------------------------------- | -------------------- | ------------------------------------------------------------------------------------------- |
| O recorte embutido do dotaconstants não pode ser lido | `UNCLASSIFIED_ERROR` | Erro no log; os itens aparecem com o nome interno e montagens de itens não são reconhecidas |
| Item que o recorte não tem                            | —                    | O item aparece com o nome interno enviado pelo GSI                                          |

### IDs das habilidades (`AbilityIdDataError`)

| Situação                                        | Código               | Diagnóstico                                                                   |
| ----------------------------------------------- | -------------------- | ----------------------------------------------------------------------------- |
| O recorte embutido não é JSON válido            | `UNCLASSIFIED_ERROR` | "Dados das habilidades indisponíveis": a ordem das habilidades fica sem nomes |
| Uma chave do recorte tem um ID que não é número | `UNCLASSIFIED_ERROR` | O mesmo diagnóstico, com a chave inválida nos detalhes técnicos               |

### Patches (`PatchDataError`)

| Situação                             | Código               | Diagnóstico                                                                          |
| ------------------------------------ | -------------------- | ------------------------------------------------------------------------------------ |
| O recorte embutido não pode ser lido | `UNCLASSIFIED_ERROR` | "Dados dos patches indisponíveis": o patch atual só aparece com os dados do OpenDota |

### Reenvio do log GSI (`replay_gsi_log`)

Ferramenta de desenvolvimento. Falhas ao ler a configuração usam os códigos da inicialização; falhas ao ler o arquivo do log ou ao iniciar o envio aparecem como `UNCLASSIFIED_ERROR`, com o título "Reenvio do log GSI falhou". O resumo final mostra só a contagem por resposta HTTP, nunca o conteúdo dos eventos.
