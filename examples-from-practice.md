### ~/.gitconfig
```
cia = "!f() { \
    msg=$(git rev-parse --show-toplevel)'/.git/.gitmessage.txt'; \
    git add \"$@\";\
    git diff --cached | gia 'Generate conventional commit message. Use Emojis in subject (Gitmoji). Do NOT explain your Procedure.' > \"$msg\" &&\
    git ci --edit -F \"$msg\";\
    rm \"$msg\";\
}; f"
```


### AutoHotKey Keyboard shortcut
```ahk
!ü::
RunWait, gia -t transkribiere -ao ,,hide
Send, ^v
Return

!Ü::
RunWait, gia -t transkribiere -ao translate to english ,,hide
Send, ^v
Return
```

### `~\.gia\tasks\transkribiere.md`
```
- transkribiere, möglichst wenig Änderungen.
- keine time codes mit ausgeben
- Keine Erklärungen.
- Keine Förmlichkeiten.
- Versuch auch gesprochene Emojis zu erkennen und zu verwenden.
- KEIN markdown.
```
