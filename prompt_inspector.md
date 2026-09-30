# Advanced Media Inspector - Módulo de Inspeção Profunda (Rust + FFI)

## Objetivo
Atualmente o sistema de importação pega apenas informações superficiais da mídia (resolução, duração). Precisamos expandir o módulo `aether_media` e os modelos do `aether_core` para realizarem uma inspeção profunda do arquivo de vídeo/áudio durante a importação.

## Requisitos (Acceptance Criteria)

### R1. Expansão do Modelo de Dados (`aether_core` e `aether_media`)
Atualizar a estrutura `MediaInspection` (e por consequência `MediaMetadata` e a estrutura do FFI) para incluir os seguintes campos:
- **Codec de Vídeo**: ex: "H.264", "HEVC", "ProRes".
- **Codec de Áudio**: ex: "AAC", "Opus".
- **Pixel Format**: ex: "yuv420p", "nv12".
- **Frame Rate Type**: Enum ou booleano indicando se é CFR (Constant Frame Rate) ou VFR (Variable Frame Rate).
- **Tabela de Keyframes**: Uma lista (array) mapeando o PTS (Presentation Time Stamp) dos keyframes (I-frames) do vídeo. Isso é crítico para acelerar a busca no decoder posteriormente.

### R2. Implementação da Inspeção (`aether_media`)
Na função `inspect_media_file`, implementar a lógica de extração dessas informações avançadas.
- Como o sistema está focando inicialmente no Mac (usando `AVFoundation` para vídeo via C/Objective-C), será necessário expandir o arquivo `avfoundation_bridge.m` para expor uma função que leia as tracks de vídeo/áudio do arquivo e retorne os codecs.
- O detector deve iterar sobre a track de vídeo (usando `AVAssetReader` ou simplesmente lendo os metadados da track via `AVAssetTrack`) para extrair a lista de timestamps dos Keyframes (I-frames) e verificar se o delta de tempo entre eles flutua (o que classificaria o vídeo como VFR).

### R3. Atualização da Ponte FFI e Flutter
- Atualizar a API do `aether_bridge` para expor as novas informações na struct retornada.
- Rodar o gerador de ponte FFI.
- Garantir que o código no Flutter compila normalmente (não é necessário desenhar a UI de propriedades agora, apenas garantir que o Dart receba os dados).

## Observações
O foco desta tarefa é exclusivamente enriquecer a análise do arquivo no momento da importação e indexar os keyframes no Rust, preparando o terreno para o player fluido.
