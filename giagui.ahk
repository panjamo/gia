#NoEnv
#SingleInstance Force
SendMode Input
SetWorkingDir %A_ScriptDir%

; GIA GUI Wrapper
; A graphical interface for the GIA command-line tool

; Create main GUI
Gui, +Resize
Gui, Font, s10, Segoe UI
Gui, Margin, 10, 10

; Add prompt input
Gui, Add, Text, x10 y10, Prompt:
Gui, Add, Edit, x10 y30 w560 h100 vPrompt gPromptEdit

; Add options group
Gui, Add, GroupBox, x10 y140 w560 h80, Options

; Add checkboxes
Gui, Add, Checkbox, x20 y160 vUseClipboard, Use clipboard input (-c)
Gui, Add, Checkbox, x20 y185 vBrowserOutput, Browser output (--browser-output)
Gui, Add, Checkbox, x300 y160 vResume, Resume last conversation (-R)

; Add response output
Gui, Add, Text, x10 y230, Response:
Gui, Add, Edit, x10 y250 w560 h295 vResponse ReadOnly Multi

; Add buttons
Gui, Add, Button, x10 y555 w100 h30 gSendPrompt Default, Send
Gui, Add, Button, x120 y555 w100 h30 gClearForm, Clear
Gui, Add, Button, x230 y555 w100 h30 gCopyResponse, Copy Response

; Add status bar
Gui, Add, Text, x10 y595 w560 vStatusBar, Ready

; Show GUI
Gui, Show, w580 h625, GIA - Google Intelligence Assistant
return

GuiClose:
ExitApp

PromptEdit:
    ; Handle Ctrl+Enter in prompt field
    if (GetKeyState("Control", "P") && GetKeyState("Enter", "P"))
        Gosub, SendPrompt
return

GuiSize:
    if (A_EventInfo = 1)  ; Minimized
        return
    
    NewWidth := A_GuiWidth
    NewHeight := A_GuiHeight
    
    ; Resize prompt
    GuiControl, Move, Prompt, % "w" . (NewWidth - 20)
    

    
    ; Resize response
    GuiControl, Move, Response, % "w" . (NewWidth - 20) . " h" . (NewHeight - 330)
    
    ; Move status bar
    GuiControl, Move, StatusBar, % "y" . (NewHeight - 30) . " w" . (NewWidth - 20)
return

SendPrompt:
    ; Get form values
    Gui, Submit, NoHide
    
    ; Validate prompt
    if (!Trim(Prompt) && !UseClipboard) {
        MsgBox, 48, Error, Please enter a prompt or select clipboard input.
        return
    }
    
    ; Build command
    Cmd := "gia.exe"
    
    ; Add flags
    if (UseClipboard)
        Cmd .= " -c"
    if (BrowserOutput)
        Cmd .= " --browser-output"
    if (Resume)
        Cmd .= " -R"
    
    ; Add prompt (replace newlines with spaces, then escape quotes)
    if (Trim(Prompt)) {
        StringReplace, CleanPrompt, Prompt, `r`n, %A_Space%, All
        StringReplace, CleanPrompt, CleanPrompt, `r, %A_Space%, All
        StringReplace, CleanPrompt, CleanPrompt, `n, %A_Space%, All
        Cmd .= " " . CleanPrompt
    }
    
    ; Update status
    GuiControl,, StatusBar, Sending request to GIA...
    GuiControl,, Response
    
    ; Execute command
    Output := RunWaitOutput(Cmd)

    ; Read output file as UTF-8
    FileEncoding, UTF-8
    FileRead, Output, %Output% 
    
    if (ErrorLevel) {
        GuiControl,, Response, % "Error: " . Output
        GuiControl,, StatusBar, Error occurred
        MsgBox, 16, Error, Failed to execute GIA command.`n`nError: %Output%
    } else {
        ; Update response
        if (BrowserOutput) {
            GuiControl,, Response, Response opened in browser!
            GuiControl,, StatusBar, Success - Response in browser
        } else {
            GuiControl,, Response, %Output%
            GuiControl,, StatusBar, Success
        }
        ; Set resume checkbox for next conversation
        GuiControl,, Resume, 1
    }
return

ClearForm:
    GuiControl,, Prompt
    GuiControl,, Response
    GuiControl,, UseClipboard, 0
    GuiControl,, BrowserOutput, 0
    GuiControl,, Resume, 0
    GuiControl,, StatusBar, Ready
return

CopyResponse:
    Gui, Submit, NoHide
    if (Trim(Response)) {
        Clipboard := Response
        GuiControl,, StatusBar, Response copied to clipboard
    }
return

; Run command and capture output
RunWaitOutput(Cmd) {
    tempFileName := A_Temp . "\gia_output.txt"
    MsgBox, 4, Confirm, Are you sure you want to execute the following command?`n`n%Cmd%`n%tempFileName%
    RunWait, cmd /c %Cmd% > %tempFileName%, , UseErrorLevel hide
    return ErrorLevel ? ErrorLevel : tempFileName
}

Trim(str) {
    return RegExReplace(str, "^\s+|\s+$")
}
