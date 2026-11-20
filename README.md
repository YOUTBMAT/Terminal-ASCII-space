# space-terminal

Um wrapper experimental que renderiza uma cena espacial colorida atrás da saída de um shell hospedado em PTY. A animação usa quatro planos visuais, ruído procedural, estrelas, um planeta, meteoros, cores RGB e renderização diferencial.

## Requisitos

- Rust 1.75 ou mais recente
- Um terminal com suporte a truecolor e caracteres Unicode

## Compilar e executar

```sh
cargo build --release
cargo run --release
```

O shell padrão vem de `$SHELL` (ou `powershell.exe` no Windows). `F5` ativa/desativa um sistema solar de fundo, com variantes visuais; `F9` ativa/desativa o warp drive; `F4` (ou `Ctrl+Alt+P`) alterna entre as paletas Deep Void, Solar Flare, Cyber Neon e Nebula Dream. As mudanças exibem por dois segundos um indicador discreto em células livres, sem cobrir texto ou cursor. `F12` ou `Ctrl+Alt+S` pausa/retoma a animação; `F10` encerra o wrapper. Teclas, digitação rápida, saída do shell, colagem, redimensionamento, mouse solicitado por TUIs, `SIGINT` e `SIGTERM` são tratados pelo processo.

## Limitações de compatibilidade

O processo filho roda em um PTY próprio e sua saída é interpretada por um emulador VT100. Ele suporta alternate screen, rolagem e margens, cores ANSI/256/truecolor, atributos comuns de texto, caracteres Unicode largos, estado/forma do cursor, bracketed paste e protocolos usuais de mouse. Células vazias de CLIs e TUIs deixam o fundo espacial visível; texto recebe apenas um sombreamento localizado e sutil para manter a leitura.

O emulador não é um terminal xterm completo: imagens, extensões específicas de cada terminal e alguns relatórios/consultas de dispositivo podem não ser suportados. Para não piscar nem aparecer em posições intermediárias durante a animação, o cursor do CLI é desenhado como um marcador estático no próprio frame; a forma de barra, bloco ou sublinhado segue o modo solicitado pelo programa.

O wrapper não pode garantir restauração diante de encerramento abrupto do processo, falha do terminal ou `SIGKILL`; os caminhos normais, panic e sinais tratáveis restauram raw mode, cursor e alternate screen.

## Desempenho

O terminal informa ao wrapper quando sua aba perde ou recupera foco (protocolo xterm focus reporting). Em foco, a animação permanece em qualidade máxima a 30 FPS mesmo sem digitação; fora de foco, reduz para 5 FPS. Com a animação pausada, o loop só redesenha quando há saída do PTY, mudança de estado ou expiração de aviso. Se o terminal hospedeiro não suportar eventos de foco, o wrapper permanece no modo de 30 FPS. O renderizador agrupa células contíguas com estilo igual e reutiliza seus buffers; o ruído da nebulosa é atualizado a cada 250 ms e reaproveitado nos frames intermediários.
