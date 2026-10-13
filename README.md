# 🌌 Aether

O **Aether** é um editor de vídeo não-linear (NLE) multiplataforma de alta performance, projetado para oferecer uma experiência fluida tanto em desktops (macOS, Linux, Windows) quanto em dispositivos móveis e tablets (iPadOS e Android).

A arquitetura do projeto une o melhor de dois mundos: **Flutter** para uma interface de usuário reativa, customizável e moderna, e **Rust** para garantir processamento de mídia brutalmente rápido, segurança de memória e baixo consumo de recursos de hardware.

---

## ✨ Funcionalidades Atuais

### 🖥️ Workspace Modular NLE
- **Layout de 4 Painéis Flexíveis:** Media Pool, Preview Player, Inspector e Timeline organizados de forma ergonômica.
- **Divisores Redimensionáveis (`ResizableSplitView`):** Ajuste de proporções entre painéis com suporte a drag-and-drop e persistência de layout.
- **Presets de Área de Trabalho (`WorkspacePresetBar`):** Alternância instantânea entre configurações de trabalho focadas em **Editing**, **Color**, **Audio** e **Effects**.
- **Design System Catppuccin Mocha:** Interface escura profissional com paleta harmoniosa, alto contraste e bordas estilizadas para evitar fadiga visual.

### 📁 Gestão de Projetos & Persistência
- **Tela de Boas-Vindas (`WelcomeScreen`):** Criação e abertura de projetos com diálogo nativo de seleção de diretórios.
- **Persistência Relacional Embarcada (`.aether` / SQLite):** Armazenamento transacional ACID via `rusqlite` (modo `WAL`) para metadados do projeto, catálogo de ativos (`media_assets`), trilhas (`timeline_tracks`) e clipes (`timeline_clips`), com migração transparente e retrocompatibilidade automática para arquivos `.aether` legados em JSON.
- **Permissões Nativas de Sandbox:** Configuração completa de *entitlements* no macOS para acesso seguro ao sistema de arquivos do usuário.

### 🎞️ Media Pool, Inspeção Profunda & Indexação Temporal Binária
- **Importação de Ativos:** Suporte a arquivos de vídeo (`.mp4`, `.mov`, `.m4v`), áudio (`.wav`, `.mp3`, `.flac`, `.aac`, `.m4a`, `.ogg`) e imagens estáticas (`.png`, `.jpg`, `.webp`, `.bmp`, `.tiff`).
- **Demuxing & Inspeção Profunda ISO-BMFF/QuickTime (`isobmff.rs`):**
  - Parser nativo de átomos (`ftyp`, `moov`, `mvhd`, `trak`, `tkhd`, `mdia`, `mdhd`, `stbl`: `stsd`, `stts`, `ctts` v0/v1, `stss`, `sdtp`, `stsz`/`stz2`, `stsc`, `stco`/`co64`) sem necessidade de decodificar pixels.
  - Extração de metadados descritivos e técnicos (`MediaTechInfo`): dimensões codificadas vs. exibição (`coded_width/height` e `display_width/height`), `bit_depth`, `pixel_format`, `track_timebase` racional, codecs de vídeo (`H.264`, `HEVC`, `VP9`, `AV1`, `ProRes`) e áudio (`AAC`, `ALAC`, `PCM`, `FLAC`, `Opus`, `MP3`), taxa de amostragem, contagem e layout de canais (`Mono`, `Stereo`, `5.1`, `7.1`).
  - **Detecção de Taxa de Quadros:** Diferenciação automática entre VFR (*Variable Frame Rate*) e CFR (*Constant Frame Rate*) a partir da tabela `stts`.
- **Índice Binário de Quadros e GOP (`frame_index.rs` — `.fidx`):**
  - Layout binário fixo `#[repr(C)]` de **24 bytes** (`FrameIndexEntry`: `pts: i64`, `file_offset: u64`, `packet_size: u32`, `flags: u8` com bits `FLAG_IS_KEYFRAME` e `FLAG_IS_DISPOSABLE`, `_pad: [u8; 3]`).
  - Persistido como arquivo binário plano contíguo (`cache/index/<key>.fidx`) com cabeçalho validado de 64 bytes (`AETHFIDX`).
  - **Zero-Latency Seeking ($O(\log N)$):** Busca binária logarítmica sobre o índice contíguo para localizar instantaneamente (< 5ms mesmo em vídeos de 1h+ a 100fps) o I-Frame/IDR-Frame anterior mais próximo a qualquer timestamp de seek.
- **Gerenciador de Cache Determinístico & Sidecars (`cache.rs`, `audio_peaks.rs` & `thumbnails.rs`):**
  - Chave determinística baseada em `SHA-256(SHA-256(primeiros_64KiB) + file_size + mtime)` com escrita atômica em disco.
  - **Cache de Picos de Áudio Multi-LOD (`audio_peaks.rs` — `.peak` / `AETHPEAK` v2):**
    - Worker assíncrono em background (`spawn_waveform_cache_worker`) que consome áudio decodificado em PCM float32 (via Symphonia com fallback nativo para `AvFoundationAudioDecoder` no macOS).
    - Acumulador streaming de memória constante ($O(1)$) com sanitização de `NaN`/`Inf` e cálculo de `min`, `max` e `rms` (`AudioPeakSample` `#[repr(C)]` de 12 bytes).
    - Pirâmide multi-nível de detalhe (LOD / mipmapping de áudio 2:1) com diretório de offsets no cabeçalho para leitura seletiva de um único nível LOD direto do disco durante zoom rápido na timeline.
  - **Miniaturas da Timeline / Filmstrips (`thumbnails.rs` — `.atlas` / `AETHTHMB`):**
    - Worker assíncrono em background (`spawn_thumbnail_filmstrip_worker`) para extração de miniaturas em baixa resolução (`160×90` em **JPEG** ou **WebP**) baseadas nos **I-Frames/IDR-Frames** do índice `.fidx` ou em intervalos fixos de tempo/PTS.
    - Sequência binária indexada com tabela `ThumbnailIndexEntry` (`#[repr(C)]` de 24 bytes: `pts`, `byte_offset`, `byte_len`, `grid_col`, `grid_row`), busca binária $O(\log N)$ por timestamp (`PTS -> offset`), coordenadas UV normalizadas e exportação de spritesheet/atlas único.
- **Inserção Direta na Timeline:** Adição ágil de mídias inspecionadas diretamente às trilhas de edição.

### 📺 Player de Pré-Visualização Nativo (Preview)
- **Streaming de Quadros em Tempo Real via FFI:** Transmissão eficiente de frames brutos RGBA8 do Rust para o Flutter utilizando `StreamSink`.
- **Renderizador Otimizado (`RawImage`):** Decodificação em memória com correção de *stride* (`rowBytes`) para prevenir distorções e cisalhamentos diagonais.
- **Controles de Reprodução:**
  - Reprodução sequencial na taxa de quadros nativa da mídia (Play/Pause) com sincronização de parada determinística.
  - *Scrubbing* de baixa latência através de seek por PTS ou tempo fracionário em segundos, com identificação real de quadros-chave via `CMSampleAttachmentKey_NotSync` no AVFoundation.
  - Exibição de timecode reativo sincronizado com o playhead (PTS e formato `HH:MM:SS:FF`).

### ⏱️ Timeline Multi-trilha Não-Destrutiva
- **Suporte Multi-trilha:** Trilhas dedicadas para Vídeo, Áudio e Sobreposições (*Overlay*).
- **Base de Tempo Racional (`Rational`):** Controle preciso de tempo baseado em frações matemáticas (numerador/denominador), prevenindo desvios cumulativos de arredondamento.
- **Recálculo Dinâmico de Duração:** Atualização automática do PTS total da timeline com base nos limites e posições dos clipes inseridos.
- **Validação Matemática de Limites:** Tratamento de erros robusto contra clipes invertidos, durações nulas e sobreposições ilegais.
- **Seleção Contextual de Clipes:** Destaque de clipe ativo com integração bidirecional ao Inspector.

### 🎛️ Inspector Contextual Inteligente
- **Comutação Dinâmica de Contexto:** Exibe propriedades globais da sequência quando nada está selecionado, ou parâmetros detalhados do clipe ativo.
- **Propriedades de Vídeo:** Ajustes de Transformação (Posição X/Y, Escala, Rotação), Opacidade, Modos de Mesclagem (*Blend Modes*) e Recorte (*Crop*).
- **Propriedades de Áudio:** Controle de Ganho/Volume, Balanço (*Pan*), Afinação (*Pitch*) e Mute.
- **Painel de Color Grading Integrado:**
  - **Rodas de Cor 3-Way:** Ajustes independentes de *Lift* (sombras), *Gamma* (médios), *Gain* (altas) e *Offset* com controles 2D e ajuste de luminância.
  - **Correção Básica de Cor:** Sliders de Temperatura, Tint, Exposição, Contraste, Realces (*Highlights*), Sombras (*Shadows*), Brancos, Pretos, Saturação e Vibratilidade (*Vibrance*).
  - **Curvas de Cor (RGB & Master):** Editor gráfico interativo de curvas spline para ajuste tonal fino.

---

## 🏗️ Arquitetura de Software

O Aether segue uma arquitetura em camadas desacopladas com fronteiras rígidas de responsabilidade:

```text
+-------------------------------------------------------------------+
|                   Camada de Apresentação (Flutter)                |
|  - Telas & Painéis: Media Pool, Preview Player, Timeline, Inspector|
|  - Layout Modular: ResizableSplitView, PanelRegistry, Presets     |
|  - Design System: Catppuccin Mocha Theme                          |
+---------------------------------+---------------------------------+
                                  |
                                  v
+-------------------------------------------------------------------+
|               Camada de Estado Reativo (Riverpod 2.x)              |
|  - ProjectState, TimelineNotifier, PreviewProvider, InspectorState |
+---------------------------------+---------------------------------+
                                  |
                                  v (Ponte FFI tipada e Streams)
+-------------------------------------------------------------------+
|               Fronteira FFI (crates/aether_bridge)                |
|  - flutter_rust_bridge v2 codegen, tipos espelhados, StreamSinks  |
|  - Gerenciador de Sessões de Reprodução (SessionRegistry)         |
+-------------------+-----------------------------+-----------------+
                    |                             |
                    v                             v
+-----------------------------------+ +-----------------------------+
|    Motor de Mídia & Indexação     | |      Motor de Domínio Core    |
|       (crates/aether_media)       | |     (crates/aether_core)      |
| - Demuxing ISO-BMFF/MOV (isobmff) | | - Timeline DAG, Tracks, Clips |
| - Índice Binário #[repr(C)] (.fidx)| | - Cálculo de PTS & Timebase   |
| - Picos Multi-LOD (.peak) & .atlas| | - Persistência SQLite + JSON  |
| - Decodificação AVFoundation/Symph| | - Metadados Técnicos (TechInfo)|
+-------------------+---------------+ +-----------------------------+
                    |
                    v
+-------------------------------------------------------------------+
|           Motor de Renderização GPU (crates/aether_render)        |
|  - Pipelines de shader e aceleração gráfica via wgpu / Metal      |
+-------------------------------------------------------------------+
```

---

## 📂 Estrutura do Monorepo

O repositório é gerenciado como um *Monorepo*, garantindo versionamento sincronizado entre os crates Rust e o app Flutter:

```text
Aether/
├── apps/
│   └── aether_app/               # Aplicativo frontend em Flutter
│       ├── lib/
│       │   ├── main.dart         # Ponto de entrada da aplicação
│       │   └── src/
│       │       ├── bridge/       # Código Dart gerado pelo flutter_rust_bridge
│       │       ├── features/     # Módulos funcionais da interface
│       │       │   ├── editor/   # Layout modular, painéis e split view
│       │       │   ├── inspector/# Inspector contextual, vídeo, áudio e Color Grading
│       │       │   ├── media_pool/# Gerenciamento e importação de ativos
│       │       │   ├── preview/  # Player nativo e renderizador de frames
│       │       │   ├── timeline/ # Timeline, playhead e seleção de clipes
│       │       │   └── welcome/  # Tela inicial e criação de projetos
│       │       ├── services/     # Serviços de I/O e seletores nativos
│       │       └── theme/        # Tema Catppuccin Mocha
│       ├── test/                 # Suíte de testes de widgets e estresse do Flutter
│       └── macos/                # Configurações de plataforma e Runner macOS
│
├── crates/                       # Motores nativos em Rust
│   ├── aether_core/              # Domínio: Timeline, Tracks, Clips, MediaTechInfo, SQLite Store
│   ├── aether_media/             # ISO-BMFF, FrameIndex (.fidx), Peaks LOD (.peak), Filmstrips (.atlas), AVFoundation
│   ├── aether_render/            # Pipeline de renderização em GPU (wgpu)
│   └── aether_bridge/            # Ponte FFI (endpoints públicos para o Flutter)
│
├── tests/                        # Testes End-to-End e suíte adversarial automatizada
│   ├── e2e_runner.py             # Orquestrador de testes de integração e invariantes
│   └── run_tests.sh              # Script de disparo rápido da suíte de testes
│
├── Makefile                      # Automação de tarefas (bridge codegen, setup)
└── Cargo.toml                    # Configuração do workspace Rust
```

---

## 🧪 Qualidade, Testes & Validação Adversarial

O Aether conta com uma infraestrutura rigorosa de testes cobrindo múltiplos níveis:

1. **Rust Core & Media Unit/Stress Tests:**
   - Validação de layout binário `#[repr(C)]` de 24 bytes (`FrameIndexEntry` e `ThumbnailIndexEntry`), serialização/desserialização `.fidx` e `.atlas`, e busca binária $O(\log N)$ com tolerância a limites (`pts < 0`, entre P/B-frames, `i64::MAX`, streams vazias e orçamento < 5ms).
   - Testes de cálculo de picos de áudio (`min`, `max`, `rms`) com buffers PCM sintéticos (silêncio, onda senoidal pura com $\text{RMS} = 1/\sqrt{2}$, clipping em amplitude máxima e resistência a `NaN`/`Inf`), geração de pirâmide multi-LOD e leitura seletiva por nível.
   - Testes de extração assíncrona de filmstrips (`160×90` JPEG/WebP), montagem de spritesheet e lookup por timestamp.
   - Verificação de fingerprinting determinístico (`partial_header + file_size + mtime`) contra vetores oficiais NIST SHA-256.
   - Validação de persistência SQLite (`store.rs`) e migração automática de projetos JSON legados.
   - Fuzzing de contêineres danificados, arquivos truncados, `timescale == 0` e cabeçalhos corrompidos sem `panic!`.
   - Testes de estresse de scrubbing aleatório, decodificação multithread e ausência de vazamento de memória.
2. **Flutter Widget & Integration Tests:**
   - Testes de interação com a Timeline, seleção de clipes e propagação ao Inspector.
   - Validação de renderização de painéis em resoluções e restrições extremas.
   - Testes de estabilidade do Media Pool e montagem/desmontagem de instâncias de pré-visualização.
3. **Suíte End-to-End (E2E) Automatizada:**
   - Orquestrador em Python (`tests/run_tests.sh`) que valida os contratos de FFI, integridade de serialização e execução das ferramentas de análise estática.

---

## 🚀 Como Desenvolver e Executar

### Pré-requisitos
- **Flutter SDK:** Versão `3.3.0+` instalada e configurada no `PATH`.
- **Rust Toolchain:** Versão estável do Rust (`rustup default stable`).
- **Codegen da Ponte FFI:** `flutter_rust_bridge_codegen` versão `2.3.0`.
- **macOS / Xcode:** Recomendado para compilação com aceleração nativa via AVFoundation e Metal.

### 1. Configurar o Ambiente
Na raiz do repositório, instale o gerador da ponte:
```bash
make setup
```

### 2. Gerar o Código da Ponte FFI (flutter_rust_bridge)
Sempre que modificar funções públicas em `crates/aether_bridge/src/api.rs`:
```bash
make bridge
```

### 3. Executar o Aplicativo
Navegue até o diretório do app Flutter e inicie no ambiente macOS:
```bash
cd apps/aether_app
flutter run -d macos
```

### 4. Executar os Testes

- **Testes do Backend (Rust):**
  ```bash
  cargo test --workspace
  ```

- **Testes da Interface (Flutter):**
  ```bash
  cd apps/aether_app
  flutter test
  ```

- **Executar a Suíte E2E Completa:**
  ```bash
  ./tests/run_tests.sh
  ```

---

## 📄 Licença

Distribuído sob licença proprietária de desenvolvimento pessoal. Consulte a documentação interna para mais detalhes.
