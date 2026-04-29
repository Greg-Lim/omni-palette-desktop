#Requires AutoHotkey v2.0

OmniPalette_RegisterForOmniPalette(A_ScriptFullPath)

OmniPalette_RegisterForOmniPalette(scriptPath) {
    local localAppData := EnvGet("LOCALAPPDATA")
    if (localAppData = "") {
        return
    }

    local snapshotsRoot := localAppData "\OmniPalette\ahk-agent\scripts"
    DirCreate(snapshotsRoot)

    local scriptText := FileRead(scriptPath, "UTF-8")
    local snapshotJson := OmniPalette_SnapshotJson(scriptPath, scriptText)
    local snapshotName := OmniPalette_SnapshotFileName(scriptPath) ".json"
    local finalPath := snapshotsRoot "\" snapshotName
    local tempPath := finalPath ".tmp"

    if FileExist(tempPath) {
        FileDelete(tempPath)
    }

    FileAppend(snapshotJson, tempPath, "UTF-8-RAW")
    FileMove(tempPath, finalPath, 1)
}

OmniPalette_SnapshotJson(scriptPath, scriptText) {
    local updatedAtUnix := DateDiff(A_NowUTC, "19700101000000", "Seconds")
    return "{"
        . '"schema_version":1,'
        . '"script_path":"' . OmniPalette_EscapeJson(scriptPath) . '",'
        . '"script_text":"' . OmniPalette_EscapeJson(scriptText) . '",'
        . '"updated_at_unix":' . updatedAtUnix . ","
        . '"agent_version":"0.1.0"'
        . "}"
}

OmniPalette_SnapshotFileName(scriptPath) {
    return "script-" . OmniPalette_Fnv1a32(scriptPath)
}

OmniPalette_Fnv1a32(text) {
    local hash := 2166136261
    for codepoint in StrSplit(text) {
        hash := hash ^ Ord(codepoint)
        hash := Mod(hash * 16777619, 0x100000000)
    }
    return Format("{:08X}", hash)
}

OmniPalette_EscapeJson(value) {
    value := StrReplace(value, "\", "\\")
    value := StrReplace(value, '"', '\"')
    value := StrReplace(value, "`r", "\r")
    value := StrReplace(value, "`n", "\n")
    value := StrReplace(value, "`t", "\t")
    return value
}
