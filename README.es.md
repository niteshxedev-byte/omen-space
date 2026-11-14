<div align="center">
  <img src="images/omenspace.png" alt="Logo de OMEN Space" width="110" />

  # OMEN Space

  **Centro de control nativo en Linux para portátiles HP OMEN, Victus y Transcend.**  
  Escrito íntegramente en **Rust** con **GTK4 y Libadwaita** para un rendimiento instantáneo y mínimo consumo.

  <p>
    <a href="https://github.com/yunusemreyl/omen-space/releases/latest">
      <img src="https://img.shields.io/badge/Versi%C3%B3n-v2.2.0-e11d48?style=flat-square" alt="Versión v2.2.0" />
    </a>
    <a href="LICENSE">
      <img src="https://img.shields.io/badge/Licencia-GPL--3.0-10b981?style=flat-square" alt="Licencia" />
    </a>
    <img src="https://img.shields.io/badge/Lenguaje-Rust-f97316?style=flat-square" alt="Rust" />
    <img src="https://img.shields.io/badge/Interfaz-GTK4%20%2B%20Libadwaita-3b82f6?style=flat-square" alt="GTK4" />
  </p>

  <p>
    <a href="README.md">English</a> · <b>Español</b>
  </p>

  <br />
  <img src="images/perf.png" width="92%" alt="OMEN Space — Rendimiento y Telemetría" />
</div>

---

## Descripción general

**OMEN Space** ofrece control total del hardware en Linux mediante un daemon nativo en Rust (~3 MB) y una interfaz moderna en GTK4 + Libadwaita.

| Módulo | Funciones principales |
| :--- | :--- |
| **Rendimiento y Refrigeración** | Perfiles térmicos `Eco`, `Balanced` y `Performance`; modos de ventilador `Auto`, `Max`, curvas personalizadas con preajustes guardados, delegación directa al **EC**, limpieza de polvo, integración de NoteBook FanControl (`nbfc`) para HP Victus y protección térmica inteligente (`>= 90 °C` / `95 °C`). |
| **Protección de Inactividad y Sueño** | Monitor de inactividad por hardware (`evdev`) que rastrea teclado, ratón y panel táctil. Si no detecta actividad, muestra un diálogo de cuenta regresiva de 60 segundos ("¿Estás despierto?"), permitiendo extender el temporizador a 3 horas o apagando el equipo de forma segura (`systemctl poweroff`) para evitar sobrecalentamiento y descarga de batería. |
| **Energía Avanzada** | Límites de potencia Intel RAPL `PL1` / `PL2`, límite térmico `TCC Offset`, undervolt por MSR (con detección de `UV_LOCK`), Dynamic Boost de GPU y ajustes para AMD Ryzen SMU. |
| **Selector de Gráficos (MUX)** | Cambio entre **Hybrid (MSHybrid)**, **Discrete (dGPU)** y **Advanced Optimus** con estado de enrutamiento `eDP` y versión del controlador NVIDIA. |
| **RGB Studio** | Iluminación RGB de 4 zonas, zona única (Victus), barra de luz frontal y teclado por tecla Darfon (`0d62:54bf`). |
| **Superposición Rápida (`Shift+F2`) y Teclas Físicas** | Panel flotante dentro del juego, applet de bandeja y asignación de la **tecla física OMEN** (`KEY_PROG1`) para abrir la app idéntico a Windows. |
| **Gestor de Perfiles y Actualizador** | Reglas automáticas por aplicación (Wayland y X11) y **Centro de Actualizaciones** integrado con seguimiento de commits, progreso en vivo y soporte para `fwupd` (LVFS). |

> **Detalles de Hardware y Nuevas Características:** Consulta [MODIFICATIONS.md](MODIFICATIONS.md) para el análisis completo de las modificaciones para HP Victus y la arquitectura de Sleep Guard.

---

## Nuevas Funciones y Adaptaciones de Hardware (Por qué y Cómo)

### 1. Backend EC con NoteBook FanControl (`nbfc`) (HP Victus 15)
- **Por qué se implementó:**
  Los portátiles HP Victus 15 (como Board ID `8A4F` / BIOS `F.28`) y modelos similares no admiten escrituras directas por HP WMI (`0x2E`), y el controlador del kernel de Linux solo expone sensores de lectura RPM sin un nodo `pwm1`. Sin esto, el control manual de ventiladores y las curvas fallaban en Linux.
- **Cómo funciona:**
  Se integró el servicio NoteBook FanControl (`nbfc`) como backend del Embedded Controller (EC). Cuando `pwm1` no está disponible, los objetivos se envían a los registros EC `0x2D` y `0x2C`, habilitando control manual (`omen-cli fan set-speed <0-100>`), modo máximo (hasta 6000 RPM) y curvas automáticas.

### 2. Protección de Inactividad y Sueño ("¿Estás despierto?")
- **Por qué se implementó:**
  Los portátiles gaming con tarjetas NVIDIA RTX consumen gran cantidad de energía y generan calor si se dejan encendidos sin supervisión (por ejemplo, si el usuario se queda dormido). La suspensión moderna (S0ix) en Linux a menudo falla o es bloqueada por controladores.
- **Cómo funciona:**
  - **Detección por Evdev:** El daemon monitorea `/dev/input/event*` directamente a nivel root (teclados, ratones y trackpads). Cualquier pulsación o movimiento reinicia el contador.
  - **Diálogo Interactivo:** Al alcanzar el límite (por defecto 2 horas; configurable de 5m a 8h), aparece un diálogo con 60 segundos de cuenta atrás:
    - **"Sí, estoy despierto (+1h a 3h)"**: Extiende el límite a 3 horas (10,800s) y notifica al usuario.
    - **"Apagar ahora"** o fin del tiempo: Apaga el sistema de forma segura (`systemctl poweroff`).
  - **Interfaz dedicada:** Pestaña **Sleep Guard** en la barra lateral con indicador en vivo, barra de progreso y selector de minutos exactos, además de una tarjeta de estado en la pantalla de Rendimiento.

### 3. Tecla Física OMEN del Teclado
- **Por qué se implementó:**
  En Windows, la tecla física con el rombo OMEN abre el OMEN Gaming Hub. En Linux, dicha tecla (`KEY_PROG1` / código 148) estaba inactiva por defecto.
- **Cómo funciona:**
  El daemon escucha el evento HP WMI y emite `MacroKeyPressed("omen")` por D-Bus, y el servicio de usuario `omen-tray` inicia `omen-gui` al instante.

### 4. Actualización Fluida de 1 Segundo en la Interfaz
- **Por qué se implementó:**
  La interfaz se actualizaba cada 2 segundos, provocando que el contador de inactividad saltara de dos en dos (`0s → 2s → 4s → 6s → 8s`).
- **Cómo funciona:**
  Se ajustó el ciclo de consulta a 1 segundo para mostrar el avance consecutivo de cada segundo.

---

## Instalación rápida

```bash
curl -sSL https://raw.githubusercontent.com/yunusemreyl/omen-space/main/install.sh | sudo bash -s -- --stable
```

Para instalar directamente desde la rama `main` (canary):

```bash
curl -sSL https://raw.githubusercontent.com/yunusemreyl/omen-space/main/install.sh | sudo bash -s -- --canary
```

---

## Galería

<p align="center">
  <img src="images/keyboard.png" width="49%" alt="RGB Studio" />
  <img src="images/undervolt.png" width="49%" alt="Energía Avanzada" />
</p>
<p align="center">
  <img src="images/mux.png" width="49%" alt="Selector de Gráficos (MUX)" />
  <img src="images/appprofiles.png" width="49%" alt="Gestor de Perfiles" />
</p>
<p align="center">
  <img src="images/updater.png" width="49%" alt="Centro de Actualizaciones" />
  <img src="images/diagnostic.png" width="49%" alt="Monitor del Sistema" />
</p>

---

## Licencia

Distribuido bajo la licencia **GPL-3.0**.  
*OMEN Space es un proyecto independiente de código abierto y no está afiliado ni respaldado por HP Inc.*
