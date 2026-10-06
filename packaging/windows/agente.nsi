; Instalador de Resguardo Agente (fase 5, docs/agente-gestionado.md).
;
; Lo genera scripts/build-agente.mjs (npm run build:agente) con:
;   makensis /INPUTCHARSET UTF8 /DVERSION=… /DAGENT_EXE=… /DRESTIC_EXE=…
;            /DRESTIC_LICENSE=… /DAGENT_SHA256=… /DOUT_FILE=… agente.nsi
;
; Instalación para todo el equipo (administrador): copia resguardo-agente.exe
; y restic.exe, instala y arranca el servicio «ResguardoAgente», y empareja el
; equipo con la consola (página «Emparejar» con el código y el número de
; comprobación en grande).
;
; Sin ventanas (para desplegar en muchos equipos):
;   Resguardo-Agente-setup.exe /S /CODE=ABCD-EFGH-JK [/TRAY=0]
;
; Actualización automática (docs/actualizaciones.md): el actualizador del
; agente lo ejecuta con /S /ACTUALIZACION=1. Igual que una actualización a
; mano, sin tocar la configuración, las claves ni los vínculos; además deja el
; icono de la bandeja como estaba (sin /TRAY=) y no intenta vincular nada.
; El número de comprobación queda en «emparejamiento.txt» junto al programa
; (solo administradores). Código de salida 2 si el emparejamiento falla.
;
; Instalador «listo» (v1.17): Resguardo Server añade al final de este mismo
; instalador una cola con servidor, huella de su autoridad TLS, cliente,
; nombre y un código de un solo uso (crates/protocolo/src/instalador.rs).
; Si la trae, tras instalar se vincula solo (también con /S) y enseña el
; número de comprobación; sin ella, todo es como siempre.
;
; Imagen y textos comunes con el de Resguardo Server: comun.nsh y arte/.
;
; Vista previa (solo para ver las páginas y hacer capturas; nunca se publica):
;   makensis /INPUTCHARSET UTF8 /DVISTA_PREVIA[=vinculado] /DOUT_FILE=… agente.nsi
; No pide ser administrador ni instala nada: la página de instalación solo
; rellena datos de ejemplo.

Unicode true
ManifestDPIAware true
!ifdef VISTA_PREVIA
  RequestExecutionLevel user
!else
  RequestExecutionLevel admin
!endif
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "nsDialogs.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"
!include "WinVer.nsh"
!include "StrFunc.nsh"
${StrStr}

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define PRODUCT "Resguardo Agente"
!define SERVICE "ResguardoAgente"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\ResguardoAgente"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
; Tarea y regla del cortafuegos del Servidor de copias del agente
; (crates/agente/src/server.rs, TASK_NAME_AGENTE).
!define SERVER_TASK "Resguardo Agente Servidor de copias"

Name "${PRODUCT}"
OutFile "${OUT_FILE}"
InstallDir "$PROGRAMFILES64\Resguardo Agente"
BrandingText "${PRODUCT} ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1034 "ProductName" "${PRODUCT}"
VIAddVersionKey /LANG=1034 "FileDescription" "Instalar ${PRODUCT}"
VIAddVersionKey /LANG=1034 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1034 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1034 "LegalCopyright" "© 2026 Ever Caro y colaboradores · AGPL-3.0-or-later"

!include "comun.nsh"
!insertmacro ArteAltaResolucionFn ""
!insertmacro ArteAltaResolucionFn "un."

Var Code
Var Servidor
Var ServidorInput
Var Tray
Var Sas
Var Console
Var CodeInput
Var TrayBox
Var PairSkipped
Var BigFont
Var CopyServer
; Instalador «listo para vincular» (v1.17): trae al final servidor, nombre y código.
Var Preparado
Var NombrePrep
Var ErrorPrep
; /ACTUALIZACION=1: lo lanza el actualizador automático del agente.
Var Actualizacion

; Texto de la página final (depende de si el equipo quedó vinculado).
Var FinTexto
; La consola que se ofrece abrir al terminar (solo https://), o nada.
Var FinConsola

!define MUI_WELCOMEPAGE_TITLE "Instalar ${PRODUCT}"
!define MUI_WELCOMEPAGE_TEXT "Resguardo Agente hace las copias de seguridad de este equipo tal como las decide quien lo gestiona desde la consola de Resguardo.$\r$\n$\r$\nNo tiene ventana: es un servicio de Windows y, si quieres, un icono discreto en la bandeja que dice cómo van las copias y quién gestiona el equipo.$\r$\n$\r$\nTen a mano el código y la dirección que muestra la consola en «Equipos → Añadir equipo». Con el instalador listo que se descarga de la consola no hace falta: se vincula solo."
!insertmacro MUI_PAGE_WELCOME
Page custom OptionsPage OptionsLeave
!insertmacro PaginaInstalar
Page custom PairPage PairLeave
Page custom SasPage
!define MUI_PAGE_CUSTOMFUNCTION_PRE FinPre
!define MUI_PAGE_CUSTOMFUNCTION_SHOW FinShow
!define MUI_FINISHPAGE_TITLE "Resguardo Agente está en marcha"
!define MUI_FINISHPAGE_TEXT "$FinTexto"
!define MUI_FINISHPAGE_TEXT_LARGE
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Abrir la consola ahora"
!define MUI_FINISHPAGE_RUN_FUNCTION AbrirConsola
; Reemplazar el programa no necesita reiniciar (el anterior se aparta y se borra al reiniciar).
!define MUI_FINISHPAGE_NOREBOOTSUPPORT
!insertmacro MUI_PAGE_FINISH

!insertmacro PaginasDesinstalar

!insertmacro MUI_LANGUAGE "Spanish"

; ---------- Opciones de la línea de órdenes ----------

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Resguardo Agente necesita Windows de 64 bits." /SD IDOK
    SetErrorLevel 5
    Abort
  ${EndIf}
  ; restic y rclone necesitan Windows 10 o Windows Server 2016 (o posteriores).
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP "Resguardo Agente necesita Windows 10 o Windows Server 2016, o una versión posterior.$\r$\n$\r$\nEste equipo tiene una versión de Windows más antigua: actualízala y vuelve a abrir el instalador." /SD IDOK
    SetErrorLevel 5
    Abort
  ${EndIf}
  SetRegView 64
  StrCpy $Tray "1"
  StrCpy $PairSkipped "0"
  StrCpy $Preparado "0"
  StrCpy $ErrorPrep ""
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/CODE=" $Code
  ${If} ${Errors}
    StrCpy $Code ""
  ${EndIf}
  ; Resguardo Server: con /SERVIDOR=https://… se vincula a él en vez de a la consola de Windows.
  ClearErrors
  ${GetOptions} $R0 "/SERVIDOR=" $Servidor
  ${If} ${Errors}
    StrCpy $Servidor ""
  ${EndIf}
  ClearErrors
  ${GetOptions} $R0 "/TRAY=" $R1
  ${IfNot} ${Errors}
  ${AndIf} $R1 == "0"
    StrCpy $Tray "0"
  ${EndIf}
  ; Actualización automática: la bandeja como estaba (si no estaba en «Ejecutar», no se pone).
  StrCpy $Actualizacion "0"
  ClearErrors
  ${GetOptions} $R0 "/ACTUALIZACION=" $R2
  ${IfNot} ${Errors}
  ${AndIf} $R2 == "1"
    StrCpy $Actualizacion "1"
    ClearErrors
    ${GetOptions} $R0 "/TRAY=" $R1
    ${If} ${Errors}
      ClearErrors
      ReadRegStr $R3 HKLM "${RUN_KEY}" "ResguardoAgente"
      ${If} $R3 == ""
        StrCpy $Tray "0"
      ${EndIf}
    ${EndIf}
  ${EndIf}
FunctionEnd

; Solo letras, cifras y guiones (va en una línea de órdenes): 1 si vale.
Function CheckCode
  Exch $R0
  Push $R1
  Push $R2
  Push $R3
  StrLen $R1 $R0
  StrCpy $R3 "1"
  ${If} $R1 < 8
  ${OrIf} $R1 > 20
    StrCpy $R3 "0"
  ${EndIf}
  StrCpy $R2 0
  ${DoWhile} $R2 < $R1
    StrCpy $R4 $R0 1 $R2
    ${StrStr} $R5 "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-" $R4
    ${If} $R5 == ""
      StrCpy $R3 "0"
    ${EndIf}
    IntOp $R2 $R2 + 1
  ${Loop}
  StrCpy $R0 $R3
  Pop $R3
  Pop $R2
  Pop $R1
  Exch $R0
FunctionEnd

; ---------- Servidor de copias (rest-server) ----------

; Para la tarea del Servidor de copias y el rest-server que lanzó (por el PID
; que guarda el agente y solo si ese proceso es rest-server.exe), para poder
; reemplazar rest-server.exe al actualizar o quitarlo al desinstalar.
!macro StopCopyServerFn un
Function ${un}StopCopyServer
  ; Primero la tarea: si no, volvería a lanzar rest-server.
  nsExec::Exec '"$SYSDIR\schtasks.exe" /End /TN "${SERVER_TASK}"'
  Pop $0
  ClearErrors
  FileOpen $1 "$APPDATA\ResguardoAgente\privado\servidor.pid" r
  ${IfNot} ${Errors}
    FileRead $1 $2
    FileClose $1
    ; Solo las cifras (sin el salto de línea).
    IntOp $2 $2 + 0
    ${If} $2 > 0
      nsExec::Exec '"$SYSDIR\taskkill.exe" /F /FI "PID eq $2" /FI "IMAGENAME eq rest-server.exe"'
      Pop $0
    ${EndIf}
  ${EndIf}
  Sleep 1000
FunctionEnd
!macroend
!insertmacro StopCopyServerFn ""
!insertmacro StopCopyServerFn "un."

; ---------- Página de opciones ----------

Function OptionsPage
  !insertmacro MUI_HEADER_TEXT "Opciones" "Lo que verá quien use este equipo."
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 0 100% 36u "El icono de la bandeja dice cómo van las copias («Tus archivos están protegidos», cuándo fue la última y cuándo toca la siguiente) y quién gestiona el equipo. No permite cambiar nada de las copias."
  Pop $0
  ${NSD_CreateCheckbox} 0 44u 100% 12u "Mostrar el icono de Resguardo en la bandeja"
  Pop $TrayBox
  ${If} $Tray == "1"
    ${NSD_Check} $TrayBox
  ${EndIf}
  ${NSD_CreateLabel} 0 66u 100% 36u "La consola puede ocultarlo o mostrarlo más adelante."
  Pop $0
  nsDialogs::Show
FunctionEnd

Function OptionsLeave
  ${NSD_GetState} $TrayBox $0
  ${If} $0 == ${BST_CHECKED}
    StrCpy $Tray "1"
  ${Else}
    StrCpy $Tray "0"
  ${EndIf}
FunctionEnd

; ---------- Instalación ----------

Section "Resguardo Agente" SecMain
  SectionIn RO
  SetShellVarContext all
  SetRegView 64
!ifdef VISTA_PREVIA
  ; Solo para ver las páginas: datos de ejemplo, nada instalado.
  DetailPrint "Vista previa: no se instala nada."
  !if "${VISTA_PREVIA}" == "vinculado"
    StrCpy $Preparado "1"
    StrCpy $Servidor "https://copias.ejemplo.com:8443"
    StrCpy $Console "Resguardo Server (copias.ejemplo.com)"
    StrCpy $Sas "482 913"
  !endif
!else

  ; Actualización: parar el servicio y la bandeja antes de reemplazar el programa.
  DetailPrint "Parando el servicio (si estaba en marcha)..."
  ; Marca de actualización: el agente no lo cuenta como «detenido por un administrador».
  ${If} ${FileExists} "$APPDATA\ResguardoAgente\*.*"
    FileOpen $9 "$APPDATA\ResguardoAgente\actualizando" w
    FileWrite $9 "${VERSION}"
    FileClose $9
  ${EndIf}
  nsExec::Exec '"$SYSDIR\sc.exe" stop ${SERVICE}'
  Pop $0
  Sleep 2000
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM resguardo-agente.exe'
  Pop $0
  ; ¿Estaba activado el Servidor de copias? Se para y se vuelve a arrancar al final.
  nsExec::Exec '"$SYSDIR\schtasks.exe" /Query /TN "${SERVER_TASK}"'
  Pop $CopyServer
  ${If} $CopyServer == 0
    DetailPrint "Parando el Servidor de copias para actualizarlo..."
    Call StopCopyServer
  ${EndIf}

  SetOutPath "$INSTDIR"
  ; Si el programa anterior sigue bloqueado un momento, se aparta (renombrar
  ; funciona aunque esté en uso) para que el nuevo se escriba seguro.
  Delete "$INSTDIR\resguardo-agente.exe.anterior"
  ${If} ${FileExists} "$INSTDIR\resguardo-agente.exe"
    Rename "$INSTDIR\resguardo-agente.exe" "$INSTDIR\resguardo-agente.exe.anterior"
  ${EndIf}
  File "/oname=resguardo-agente.exe" "${AGENT_EXE}"
  Delete /REBOOTOK "$INSTDIR\resguardo-agente.exe.anterior"
  File "/oname=restic.exe" "${RESTIC_EXE}"
  ; rest-server oficial (para «Este equipo guarda copias»; el agente comprueba su huella antes de arrancarlo).
  !ifdef REST_SERVER_EXE
    Delete "$INSTDIR\rest-server.exe.anterior"
    ${If} ${FileExists} "$INSTDIR\rest-server.exe"
      Rename "$INSTDIR\rest-server.exe" "$INSTDIR\rest-server.exe.anterior"
    ${EndIf}
    File "/oname=rest-server.exe" "${REST_SERVER_EXE}"
    Delete /REBOOTOK "$INSTDIR\rest-server.exe.anterior"
  !endif
  ; rclone oficial (espejo del Servidor de copias en una nube; el agente comprueba su huella antes de usarlo).
  !ifdef RCLONE_EXE
    File "/oname=rclone.exe" "${RCLONE_EXE}"
  !endif
  SetOutPath "$INSTDIR\licenses"
  File "/oname=restic-LICENSE.txt" "${RESTIC_LICENSE}"
  !ifdef REST_SERVER_LICENSE
    File "/oname=rest-server-LICENSE.txt" "${REST_SERVER_LICENSE}"
  !endif
  !ifdef RCLONE_LICENSE
    File "/oname=rclone-LICENSE.txt" "${RCLONE_LICENSE}"
  !endif
  SetOutPath "$INSTDIR"

  ; El programa es el que se compiló con este instalador (huella fijada).
  !ifdef AGENT_SHA256
    nsExec::ExecToStack '"$SYSDIR\certutil.exe" -hashfile "$INSTDIR\resguardo-agente.exe" SHA256'
    Pop $0
    Pop $1
    ${StrStr} $2 $1 "${AGENT_SHA256}"
    ${If} $2 == ""
      MessageBox MB_ICONSTOP "El programa instalado no coincide con el de este instalador. Descarga de nuevo el instalador desde la consola." /SD IDOK
      Delete "$INSTDIR\resguardo-agente.exe"
      SetErrorLevel 3
      Abort
    ${EndIf}
  !endif

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "Resguardo"
  ; El icono de Resguardo (resguardo-agente.exe no lleva icono; el desinstalador, sí).
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\uninstall.exe,0"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1

  DetailPrint "Instalando el servicio ${SERVICE}..."
  nsExec::Exec '"$INSTDIR\resguardo-agente.exe" --install-service'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "No se pudo instalar el servicio de Resguardo Agente (código $0).$\r$\n$\r$\nVuelve a abrir el instalador. Si se repite, reinicia el equipo y prueba otra vez; el motivo queda en el registro del agente ($APPDATA\ResguardoAgente\agent.log)." /SD IDOK
    SetErrorLevel 4
    Abort
  ${EndIf}

  ; El Servidor de copias otra vez en marcha (si estaba activado).
  ${If} $CopyServer == 0
    nsExec::Exec '"$SYSDIR\schtasks.exe" /Run /TN "${SERVER_TASK}"'
    Pop $0
  ${EndIf}

  ${If} $Tray == "1"
    WriteRegStr HKLM "${RUN_KEY}" "ResguardoAgente" '"$INSTDIR\resguardo-agente.exe" --tray'
    ${IfNot} ${Silent}
      Exec '"$INSTDIR\resguardo-agente.exe" --tray'
    ${EndIf}
  ${Else}
    DeleteRegValue HKLM "${RUN_KEY}" "ResguardoAgente"
  ${EndIf}

  ; Instalador «listo» (descargado de la consola): se vincula aquí, sin preguntar nada.
  ; En una actualización automática no se vincula nada (el equipo ya lo está, o no se toca).
  ${If} $Actualizacion != "1"
    Call PairPreparado
  ${EndIf}

  ; Sin ventanas: se empareja aquí con /CODE=.
  Call AlreadyPaired
  Pop $0
  ${If} $Preparado == "1"
    ; Ya se intentó con los datos del instalador.
  ${ElseIf} ${Silent}
  ${AndIf} $Code != ""
  ${AndIf} $0 == "0"
    Call PairNow
    ${If} $Sas == ""
      SetErrorLevel 2
    ${EndIf}
  ${EndIf}
!endif
SectionEnd

!macro TrimNewLineCall out in
  Push "${in}"
  Call TrimNewLineFn
  Pop "${out}"
!macroend
!define TrimNewLine "!insertmacro TrimNewLineCall"

Function TrimNewLineFn
  Exch $R0
  Push $R1
  loop:
    StrCpy $R1 $R0 1 -1
    ${If} $R1 == "$\r"
    ${OrIf} $R1 == "$\n"
      StrCpy $R0 $R0 -1
      Goto loop
    ${EndIf}
  Pop $R1
  Exch $R0
FunctionEnd

; Ejecuta --pair con $Code. Deja el número en $Sas y la consola en $Console;
; si falla, $Sas vacío y el motivo en $R9.
Function PairNow
  StrCpy $Sas ""
  StrCpy $Console ""
  StrCpy $R9 ""
  Push $Code
  Call CheckCode
  Pop $0
  ${If} $0 != "1"
    StrCpy $R9 "Ese código no es válido: son letras y cifras, como ABCD-EFGH-JK."
    Return
  ${EndIf}
  InitPluginsDir
  Delete "$PLUGINSDIR\pair.txt"
  ${If} $Servidor != ""
    ; Solo https:// y sin comillas ni espacios (va en una línea de órdenes).
    StrCpy $1 $Servidor 8
    ${StrStr} $2 $Servidor '"'
    ${StrStr} $3 $Servidor " "
    ${If} $1 != "https://"
    ${OrIf} $2 != ""
    ${OrIf} $3 != ""
      StrCpy $R9 "La dirección del servidor debe empezar por https:// (p. ej. https://192.168.1.20:8443)."
      Return
    ${EndIf}
    ExecWait '"$INSTDIR\resguardo-agente.exe" vincular "$Code" --servidor "$Servidor" --result "$PLUGINSDIR\pair.txt"' $0
    ; Recién instalado, el servicio aún arranca: un segundo intento si el primero falla
    ; (el motivo queda en el registro del agente).
    ${If} $0 != 0
      Sleep 3000
      ExecWait '"$INSTDIR\resguardo-agente.exe" vincular "$Code" --servidor "$Servidor" --result "$PLUGINSDIR\pair.txt"' $0
    ${EndIf}
  ${Else}
    ExecWait '"$INSTDIR\resguardo-agente.exe" --pair "$Code" --result "$PLUGINSDIR\pair.txt"' $0
  ${EndIf}
  ClearErrors
  FileOpen $1 "$PLUGINSDIR\pair.txt" r
  ${If} ${Errors}
    StrCpy $R9 "No se pudo emparejar (el agente terminó con el código $0). Comprueba que este equipo llega al servidor y vuelve a intentarlo; el motivo queda en el registro del agente."
    ${If} $Servidor == ""
      StrCpy $R9 "No se pudo emparejar: falta la dirección de la consola (Resguardo Server), la que sale junto al código, p. ej. https://192.168.1.20:8443. Sin dirección solo se empareja con la antigua app de escritorio."
    ${EndIf}
    Return
  ${EndIf}
  FileRead $1 $2
  FileRead $1 $3
  FileRead $1 $4
  FileClose $1
  ; Sin el salto de línea final.
  ${TrimNewLine} $2 $2
  ${TrimNewLine} $3 $3
  ${TrimNewLine} $4 $4
  ${If} $2 == "ok"
    StrCpy $Sas $3
    StrCpy $Console $4
    FileOpen $1 "$INSTDIR\emparejamiento.txt" w
    FileWrite $1 "Consola: $Console$\r$\nNúmero de comprobación: $Sas$\r$\n"
    FileClose $1
  ${Else}
    StrCpy $R9 $3
  ${EndIf}
FunctionEnd

; ---------- Emparejar ----------

; ¿Ya emparejado? (al actualizar): no se vuelve a pedir el código.
Function AlreadyPaired
  !ifdef VISTA_PREVIA
    Push "0"
    Return
  !endif
  SetShellVarContext all
  ${If} ${FileExists} "$APPDATA\ResguardoAgente\privado\gestionado.bin"
  ${OrIf} ${FileExists} "$APPDATA\ResguardoAgente\privado\servidor.bin"
    Push "1"
  ${Else}
    Push "0"
  ${EndIf}
FunctionEnd

; Instalador «listo» (v1.17): si el propio instalador trae al final los datos
; para vincular (servidor, huella de su autoridad TLS, cliente, nombre y un
; código de un solo uso), el agente los lee y se vincula. Sin ellos, nada.
Function PairPreparado
  StrCpy $Preparado "0"
  StrCpy $ErrorPrep ""
  Call AlreadyPaired
  Pop $0
  ${If} $0 == "1"
    Return
  ${EndIf}
  InitPluginsDir
  Delete "$PLUGINSDIR\inst.txt"
  nsExec::Exec '"$INSTDIR\resguardo-agente.exe" leer-instalador "$EXEPATH" --result "$PLUGINSDIR\inst.txt"'
  Pop $0
  ${If} $0 != 0
    Return
  ${EndIf}
  ClearErrors
  FileOpen $1 "$PLUGINSDIR\inst.txt" r
  ${If} ${Errors}
    Return
  ${EndIf}
  FileRead $1 $2
  FileRead $1 $3
  FileRead $1 $4
  FileClose $1
  ${TrimNewLine} $2 $2
  ${TrimNewLine} $3 $3
  ${TrimNewLine} $4 $4
  ${If} $2 != "ok"
    Return
  ${EndIf}
  StrCpy $Preparado "1"
  StrCpy $Servidor $3
  StrCpy $NombrePrep $4
  DetailPrint "Vinculando con $Servidor como «$NombrePrep»..."
  StrCpy $Sas ""
  StrCpy $Console ""
  Delete "$PLUGINSDIR\pair.txt"
  ExecWait '"$INSTDIR\resguardo-agente.exe" vincular --instalador "$EXEPATH" --result "$PLUGINSDIR\pair.txt"' $0
  ; Recién instalado, el servicio aún arranca: un segundo intento si el primero falla.
  ${If} $0 != 0
    Sleep 3000
    ExecWait '"$INSTDIR\resguardo-agente.exe" vincular --instalador "$EXEPATH" --result "$PLUGINSDIR\pair.txt"' $0
  ${EndIf}
  ClearErrors
  FileOpen $1 "$PLUGINSDIR\pair.txt" r
  ${If} ${Errors}
    StrCpy $ErrorPrep "El agente terminó con el código $0; el motivo queda en su registro."
    Goto fallo
  ${EndIf}
  FileRead $1 $2
  FileRead $1 $3
  FileRead $1 $4
  FileClose $1
  ${TrimNewLine} $2 $2
  ${TrimNewLine} $3 $3
  ${TrimNewLine} $4 $4
  ${If} $2 == "ok"
    StrCpy $Sas $3
    StrCpy $Console $4
    FileOpen $1 "$INSTDIR\emparejamiento.txt" w
    FileWrite $1 "Consola: $Console$\r$\nEquipo: $NombrePrep$\r$\nNúmero de comprobación: $Sas$\r$\n"
    FileClose $1
    Return
  ${EndIf}
  StrCpy $ErrorPrep $3
  fallo:
  DetailPrint "No se pudo vincular con los datos del instalador: $ErrorPrep"
  ${If} ${Silent}
    SetErrorLevel 2
  ${EndIf}
FunctionEnd

Function PairPage
  ; Vinculado con los datos del instalador: directo al número de comprobación.
  ${If} $Preparado == "1"
  ${AndIf} $Sas != ""
    StrCpy $PairSkipped "0"
    Abort
  ${EndIf}
  Call AlreadyPaired
  Pop $0
  ${If} $0 == "1"
    StrCpy $PairSkipped "1"
    Abort
  ${EndIf}
  !insertmacro MUI_HEADER_TEXT "Emparejar con la consola" "El código de un solo uso que muestra la consola."
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 0 100% 30u "Escribe el código que te muestra la consola de Resguardo Server (en «Equipos → Añadir equipo»):"
  Pop $0
  ${NSD_CreateText} 0 34u 60% 16u $Code
  Pop $CodeInput
  CreateFont $1 "Consolas" 14 600
  SendMessage $CodeInput ${WM_SETFONT} $1 1
  ${NSD_CreateLabel} 0 56u 100% 12u "Dirección de la consola (Resguardo Server), la que sale junto al código:"
  Pop $0
  ${NSD_CreateText} 0 70u 100% 14u $Servidor
  Pop $ServidorInput
  ${If} $ErrorPrep != ""
    ${NSD_CreateLabel} 0 92u 100% 40u "No se pudo vincular con los datos de este instalador: $ErrorPrep  Pide en la consola un código nuevo y escríbelo arriba."
  ${Else}
    ${NSD_CreateLabel} 0 90u 100% 44u "Por ejemplo https://192.168.1.20:8443. Deja el código en blanco para emparejarlo más tarde (como administrador: resguardo-agente.exe vincular CÓDIGO --servidor https://…).$\r$\nSolo con la antigua app de escritorio (Resguardo para Windows) se deja vacía la dirección."
  ${EndIf}
  Pop $0
  ${NSD_SetFocus} $CodeInput
  nsDialogs::Show
FunctionEnd

Function PairLeave
  ${NSD_GetText} $CodeInput $Code
  ${NSD_GetText} $ServidorInput $Servidor
  ${If} $Code == ""
    StrCpy $PairSkipped "1"
    Return
  ${EndIf}
  StrCpy $PairSkipped "0"
  StrCpy $Preparado "0"
  Call PairNow
  ${If} $Sas == ""
    MessageBox MB_ICONEXCLAMATION "No se pudo emparejar:$\r$\n$\r$\n$R9"
    Abort
  ${EndIf}
FunctionEnd

Function SasPage
  ${If} $PairSkipped == "1"
    Abort
  ${EndIf}
  !insertmacro MUI_HEADER_TEXT "Comprueba el número" "Para estar seguros de que es tu consola."
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 0 100% 24u "Emparejado con «$Console». Comprueba que la consola muestra el mismo número:"
  Pop $0
  ${NSD_CreateLabel} 0 30u 100% 44u "$Sas"
  Pop $1
  CreateFont $BigFont "Segoe UI" 36 700
  SendMessage $1 ${WM_SETFONT} $BigFont 1
  ${NSD_AddStyle} $1 ${SS_CENTER}
  ${NSD_CreateLabel} 0 82u 100% 40u "Si coincide, pulsa en la consola «Coincide». Si no coincide, pulsa allí «No coincide»: alguien podría estar haciéndose pasar por ella."
  Pop $0
  nsDialogs::Show
FunctionEnd

; ---------- Página final ----------

; El siguiente paso, según cómo haya quedado el equipo.
Function FinPre
  StrCpy $FinConsola ""
  StrCpy $0 $Servidor 8
  ${If} $0 == "https://"
    StrCpy $FinConsola $Servidor
  ${EndIf}
  Call AlreadyPaired
  Pop $0
  ${If} $Sas != ""
    StrCpy $FinTexto "Este equipo está vinculado con «$Console». Sus copias empiezan en cuanto la consola le asigne qué copiar.$\r$\n$\r$\nSi aún no lo has hecho, pulsa «Coincide» en la consola."
  ${ElseIf} $0 == "1"
    StrCpy $FinTexto "Este equipo ya estaba vinculado con su consola: sigue con sus copias como hasta ahora."
  ${Else}
    StrCpy $FinTexto "Falta vincular este equipo con la consola. Como administrador, en el símbolo del sistema:$\r$\n$\r$\n  resguardo-agente vincular CÓDIGO --servidor https://…$\r$\n$\r$\nEl código y la dirección están en la consola, en «Equipos → Añadir equipo»."
  ${EndIf}
  ${If} $FinConsola != ""
    StrCpy $FinTexto "$FinTexto$\r$\n$\r$\nAbre la consola: $FinConsola"
  ${EndIf}
FunctionEnd

; Sin una consola que abrir, sin la casilla.
Function FinShow
  ${If} $FinConsola == ""
    SendMessage $mui.FinishPage.Run ${BM_SETCHECK} ${BST_UNCHECKED} 0
    ShowWindow $mui.FinishPage.Run ${SW_HIDE}
  ${EndIf}
FunctionEnd

Function AbrirConsola
  ${If} $FinConsola != ""
    !insertmacro AbrirEnElNavegador $FinConsola
  ${EndIf}
FunctionEnd

; ---------- Desinstalación ----------

Function un.onInit
  SetRegView 64
FunctionEnd

Section "Uninstall"
  SetShellVarContext all
  SetRegView 64
  DetailPrint "Parando y quitando el servicio..."
  nsExec::Exec '"$INSTDIR\resguardo-agente.exe" --uninstall-service'
  Pop $0
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM resguardo-agente.exe'
  Pop $0
  DeleteRegValue HKLM "${RUN_KEY}" "ResguardoAgente"

  ; El Servidor de copias: rest-server, su tarea y su regla del cortafuegos.
  ; Las copias que guardaba se quedan en su carpeta.
  DetailPrint "Parando y quitando el Servidor de copias (si estaba activado)..."
  Call un.StopCopyServer
  nsExec::Exec '"$SYSDIR\schtasks.exe" /Delete /TN "${SERVER_TASK}" /F'
  Pop $0
  nsExec::Exec '"$SYSDIR\netsh.exe" advfirewall firewall delete rule "name=${SERVER_TASK}"'
  Pop $0

  ; Los datos (emparejamiento, copias programadas, historial y contraseñas
  ; cifradas) solo se borran si se pide. Son solo del agente
  ; (ProgramData\ResguardoAgente): los de la app de escritorio
  ; (ProgramData\Resguardo) nunca se tocan.
  ${If} ${FileExists} "$APPDATA\ResguardoAgente\*.*"
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "¿Borrar también los datos de Resguardo Agente de este equipo?$\r$\n$\r$\n$APPDATA\ResguardoAgente guarda el emparejamiento con la consola, las copias programadas, el historial y las contraseñas (cifradas). Las copias guardadas en el servidor no se tocan.$\r$\n$\r$\nSi vas a volver a instalarlo, elige «No»." /SD IDNO IDNO keep_data
    DetailPrint "Borrando los datos del agente..."
    nsExec::Exec '"$INSTDIR\resguardo-agente.exe" --agent-purge'
    Pop $0
    keep_data:
  ${EndIf}

  Delete "$INSTDIR\resguardo-agente.exe"
  Delete "$INSTDIR\restic.exe"
  Delete "$INSTDIR\rest-server.exe"
  Delete "$INSTDIR\rest-server.exe.anterior"
  Delete "$INSTDIR\resguardo-agente.exe.anterior"
  Delete "$INSTDIR\licenses\rest-server-LICENSE.txt"
  Delete "$INSTDIR\rclone.exe"
  Delete "$INSTDIR\licenses\rclone-LICENSE.txt"
  Delete "$INSTDIR\emparejamiento.txt"
  Delete "$INSTDIR\licenses\restic-LICENSE.txt"
  RMDir "$INSTDIR\licenses"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "${UNINST_KEY}"
SectionEnd
