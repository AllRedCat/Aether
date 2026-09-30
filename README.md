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
- **Formato de Projeto Atômico (`.aether`):** Serialização e desserialização completa e não-destrutiva de timelines, trilhas, clipes e pool de ativos em JSON.
- **Permissões Nativas de Sandbox:** Configuração completa de *entitlements* no macOS para acesso seguro ao sistema de arquivos do usuário.

### 🎞️ Media Pool & Inspeção Avançada
- **Importação de Ativos:** Suporte a arquivos de vídeo, áudio e imagens estáticas.
- **Inspeção Técnica de Metadados:**
  - Extração de resolução (largura x altura), duração em segundos e PTS, FPS e aspect ratio.
  - Identificação de codecs de vídeo (ex: H.264/AVC) e áudio (ex: AAC, PCM).
  - Taxa de amostragem de áudio, quantidade de canais e tamanho em bytes.
  - **Detecção de Taxa de Quadros:** Diferenciação automática entre VFR (*Variable Frame Rate*) e CFR (*Constant Frame Rate*).
  - **Indexação de Keyframes:** Extração de tabela de PTS de I-Frames para scrubbing e saltos temporais precisos.
- **Inserção Direta na Timeline:** Adição ágil de mídias inspecionadas diretamente às trilhas de edição.

### 📺 Player de Pré-Visualização Nativo (Preview)
- **Streaming de Quadros em Tempo Real via FFI:** Transmissão eficiente de frames brutos RGBA8 do Rust para o Flutter utilizando `StreamSink`.
- **Renderizador Otimizado (`RawImage`):** Decodificação em memória com correção de *stride* (`rowBytes`) para prevenir distorções e cisalhamentos diagonais.
- **Controles de Reprodução:**
  - Reprodução sequencial na taxa de quadros nativa da mídia (Play/Pause).
  - *Scrubbing* de baixa latência através de seek por PTS ou tempo fracionário em segundos.
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
|    Motor de Mídia & Decodificação | |      Motor de Domínio Core    |
|       (crates/aether_media)       | |     (crates/aether_core)      |
| - Aceleração nativa (AVFoundation)| | - Timeline DAG, Tracks, Clips |
| - Fallbacks de Áudio (Symphonia)  | | - Cálculo de PTS & Timebase   |
| - Inspeção VFR/CFR e Keyframes    | | - Persistência (.aether JSON) |
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
│   ├── aether_core/              # Modelos de domínio: Timeline, Tracks, Clips, PTS, Project
│   ├── aether_media/             # Decodificação de mídia, inspeção profunda, AVFoundation
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
   - Validação matemática de limites e saturação de inteiros (`i64::MAX`).
   - Fuzzing de contêineres danificados, arquivos truncados e cabeçalhos corrompidos.
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
