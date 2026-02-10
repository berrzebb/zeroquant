<# .SYNOPSIS
  ZeroQuant Agent Dashboard
.DESCRIPTION
  Parses .claude/agents/*.md, agent-memory/, and git log to display
  agent configuration, memory contents, and recent work history.
.EXAMPLE
  .\scripts\show-agents.ps1
  .\scripts\show-agents.ps1 -Detailed
  .\scripts\show-agents.ps1 -Agent lead
  .\scripts\show-agents.ps1 -Memory
  .\scripts\show-agents.ps1 -History
#>
param(
    [switch]$Detailed,
    [string]$Agent,
    [switch]$Memory,
    [switch]$History
)

$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$root = (Get-Location).Path
$agentsDir = Join-Path $root ".claude\agents"
$memoryDir = Join-Path $root ".claude\agent-memory"

if (-not (Test-Path $agentsDir)) {
    Write-Host "[ERROR] Agent directory not found: $agentsDir" -ForegroundColor Red
    exit 1
}

function Parse-AgentFile {
    param([string]$FilePath)
    
    $content = Get-Content $FilePath -Raw -Encoding UTF8
    $result = @{
        File = Split-Path $FilePath -Leaf
        Name = ""
        Description = ""
        Model = ""
        Tools = ""
        DisallowedTools = ""
        PermissionMode = ""
        Memory = ""
        Skills = @()
        McpServers = @()
    }
    
    if ($content -match '(?s)^---\s*\n(.*?)\n---') {
        $yaml = $Matches[1]
        
        if ($yaml -match 'name:\s*(.+)') { $result.Name = $Matches[1].Trim() }
        if ($yaml -match 'description:\s*(.+)') { $result.Description = $Matches[1].Trim() }
        if ($yaml -match 'model:\s*(.+)') { $result.Model = $Matches[1].Trim() }
        if ($yaml -match 'tools:\s*(.+)') { $result.Tools = $Matches[1].Trim() }
        if ($yaml -match 'disallowedTools:\s*(.+)') { $result.DisallowedTools = $Matches[1].Trim() }
        if ($yaml -match 'permissionMode:\s*(.+)') { $result.PermissionMode = $Matches[1].Trim() }
        if ($yaml -match 'memory:\s*(.+)') { $result.Memory = $Matches[1].Trim() }
        
        if ($yaml -match '(?s)skills:\s*\n((?:\s+-\s+.+\n?)+)') {
            $result.Skills = ($Matches[1] -split '\n' | ForEach-Object { 
                if ($_ -match '^\s+-\s+(.+)') { $Matches[1].Trim() }
            }) | Where-Object { $_ }
        }
        if ($yaml -match '(?s)mcpServers:\s*\n((?:\s+-\s+.+\n?)+)') {
            $result.McpServers = ($Matches[1] -split '\n' | ForEach-Object { 
                if ($_ -match '^\s+-\s+(.+)') { $Matches[1].Trim() }
            }) | Where-Object { $_ }
        }
    }
    
    return $result
}

function Get-MemoryStatus {
    param([string]$AgentName)
    
    # Claude Code agent memory: folder-based (.claude/agent-memory/<agent>/*.md)
    $memDir = Join-Path $memoryDir $AgentName
    if (Test-Path $memDir) {
        $files = Get-ChildItem -Path $memDir -Filter "*.md" -ErrorAction SilentlyContinue
        if ($files.Count -gt 0) {
            $totalSize = ($files | Measure-Object -Property Length -Sum).Sum
            $size = if ($totalSize -gt 1024) { "{0:N1}KB" -f ($totalSize / 1024) } else { "{0}B" -f $totalSize }
            $latest = $files | Sort-Object LastWriteTime -Descending | Select-Object -First 1
            $lastWrite = $latest.LastWriteTime.ToString("yyyy-MM-dd HH:mm")
            return @{ Exists = $true; Size = $size; LastWrite = $lastWrite; FileCount = $files.Count; Files = $files; Dir = $memDir }
        }
    }
    # Fallback: single file (.claude/agent-memory/<agent>.md)
    $memFile = Join-Path $memoryDir "$AgentName.md"
    if (Test-Path $memFile) {
        $info = Get-Item $memFile
        $sizeVal = $info.Length
        $size = if ($sizeVal -gt 1024) { "{0:N1}KB" -f ($sizeVal / 1024) } else { "{0}B" -f $sizeVal }
        $lastWrite = $info.LastWriteTime.ToString("yyyy-MM-dd HH:mm")
        return @{ Exists = $true; Size = $size; LastWrite = $lastWrite; FileCount = 1; Files = @($info); Dir = $null }
    }
    return @{ Exists = $false; Size = "-"; LastWrite = "-"; FileCount = 0; Files = @(); Dir = $null }
}

function Get-MemoryPreview {
    param([string]$FilePath, [int]$MaxLines = 3)
    
    $lines = Get-Content $FilePath -Encoding UTF8 -TotalCount 20 -ErrorAction SilentlyContinue
    $preview = @()
    foreach ($line in $lines) {
        $trimmed = $line.Trim()
        if ($trimmed -eq "" -or $trimmed -eq "---") { continue }
        if ($trimmed -match '^#') { 
            $preview += $trimmed
            continue
        }
        if ($trimmed -match '^\*\*' -or $trimmed -match '^>' -or $trimmed -match '^-\s') {
            $preview += $trimmed
        }
        if ($preview.Count -ge $MaxLines) { break }
    }
    return $preview
}

function Get-GitHistory {
    param([string]$AgentName, [int]$MaxCommits = 5)
    
    try {
        # Search for commits mentioning agent name or Co-Authored-By Claude
        $logOutput = git log --oneline --all --grep="$AgentName" -n $MaxCommits 2>$null
        if (-not $logOutput) {
            # Fallback: recent Claude co-authored commits
            $logOutput = git log --oneline -n $MaxCommits --author="Claude" 2>$null
        }
        if (-not $logOutput) {
            $logOutput = git log --oneline -n $MaxCommits --grep="Co-Authored-By: Claude" 2>$null
        }
        if ($logOutput) {
            return ($logOutput -split '\n' | Where-Object { $_.Trim() })
        }
    } catch {}
    return @()
}

function Get-ModelIcon {
    param([string]$Model)
    switch ($Model) {
        "opus"   { return "[*]" }
        "sonnet" { return "[o]" }
        "haiku"  { return "[.]" }
        default  { return "[ ]" }
    }
}

function Get-ModelCostLabel {
    param([string]$Model)
    switch ($Model) {
        "opus"   { return "HIGH" }
        "sonnet" { return "MED " }
        "haiku"  { return "LOW " }
        default  { return "?   " }
    }
}

function Get-AccessMode {
    param($AgentData)
    if ($AgentData.PermissionMode) { return $AgentData.PermissionMode }
    if ($AgentData.DisallowedTools -match "Edit|Write") { return "readonly" }
    return "default"
}

# Collect agents
$agentFiles = Get-ChildItem -Path $agentsDir -Filter "*.md" | Sort-Object Name
$agents = $agentFiles | ForEach-Object { Parse-AgentFile $_.FullName }

# Single agent detail view
if ($Agent) {
    $target = $agents | Where-Object { $_.Name -eq $Agent }
    if (-not $target) {
        Write-Host "[ERROR] Agent '$Agent' not found." -ForegroundColor Red
        $names = ($agents | ForEach-Object { $_.Name }) -join ", "
        Write-Host "Available: $names" -ForegroundColor Yellow
        exit 1
    }
    
    $mem = Get-MemoryStatus $target.Name
    $icon = Get-ModelIcon $target.Model
    $access = Get-AccessMode $target
    
    Write-Host ""
    Write-Host "=====================================================" -ForegroundColor Cyan
    Write-Host "  $icon $($target.Name)" -ForegroundColor White
    Write-Host "=====================================================" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  Model:        $($target.Model) ($(Get-ModelCostLabel $target.Model))" -ForegroundColor White
    Write-Host "  Access:       $access" -ForegroundColor White
    Write-Host "  Tools:        $($target.Tools)" -ForegroundColor White
    if ($target.DisallowedTools) {
        Write-Host "  Disallowed:   $($target.DisallowedTools)" -ForegroundColor Red
    }
    Write-Host "  Memory:       $($target.Memory)" -ForegroundColor White
    if ($target.Skills.Count -gt 0) {
        Write-Host "  Skills:       $($target.Skills -join ', ')" -ForegroundColor Magenta
    }
    if ($target.McpServers.Count -gt 0) {
        Write-Host "  MCP Servers:  $($target.McpServers -join ', ')" -ForegroundColor Blue
    }
    Write-Host ""
    Write-Host "  --- Memory Status ---" -ForegroundColor DarkGray
    if ($mem.Exists) {
        Write-Host "  Size:         $($mem.Size) ($($mem.FileCount) file(s))" -ForegroundColor Green
        Write-Host "  Last Updated: $($mem.LastWrite)" -ForegroundColor Green
        Write-Host ""
        Write-Host "  --- Memory Contents ---" -ForegroundColor DarkGray
        foreach ($mf in $mem.Files) {
            $relName = $mf.Name
            $fSize = if ($mf.Length -gt 1024) { "{0:N1}KB" -f ($mf.Length / 1024) } else { "{0}B" -f $mf.Length }
            Write-Host "  [$relName] ($fSize, $($mf.LastWriteTime.ToString('yyyy-MM-dd HH:mm')))" -ForegroundColor Cyan
            $preview = Get-MemoryPreview $mf.FullName
            foreach ($p in $preview) {
                Write-Host "    $p" -ForegroundColor Gray
            }
        }
    } else {
        Write-Host "  No memory file (never used)" -ForegroundColor DarkGray
    }
    
    # Git history
    Write-Host ""
    Write-Host "  --- Recent Git Activity ---" -ForegroundColor DarkGray
    $gitHistory = Get-GitHistory $target.Name
    if ($gitHistory.Count -gt 0) {
        foreach ($commit in $gitHistory) {
            Write-Host "    $commit" -ForegroundColor Gray
        }
    } else {
        Write-Host "    No commits found" -ForegroundColor DarkGray
    }
    Write-Host ""
    exit 0
}

# Summary table
Write-Host ""
Write-Host "===============================================================================" -ForegroundColor Cyan
Write-Host "  ZeroQuant Agent Dashboard                          $(Get-Date -Format 'yyyy-MM-dd HH:mm')" -ForegroundColor White
Write-Host "===============================================================================" -ForegroundColor Cyan
Write-Host ""

$opusCount = ($agents | Where-Object { $_.Model -eq "opus" }).Count
$sonnetCount = ($agents | Where-Object { $_.Model -eq "sonnet" }).Count
$haikuCount = ($agents | Where-Object { $_.Model -eq "haiku" }).Count

Write-Host "  Models: [*] opus($opusCount)  [o] sonnet($sonnetCount)  [.] haiku($haikuCount)  Total: $($agents.Count)" -ForegroundColor White
Write-Host ""

# Table header
$header = "  {0,-4} {1,-15} {2,-8} {3,-5} {4,-12} {5,-18} {6,-10} {7}" -f "","AGENT","MODEL","COST","ACCESS","MCP SERVERS","MEMORY","LAST ACTIVE"
Write-Host $header -ForegroundColor DarkCyan
Write-Host "  $('-' * 86)" -ForegroundColor DarkGray

foreach ($a in $agents) {
    $icon = Get-ModelIcon $a.Model
    $cost = Get-ModelCostLabel $a.Model
    $mcps = if ($a.McpServers.Count -gt 0) { ($a.McpServers -join ",") } else { "-" }
    $access = Get-AccessMode $a
    $mem = Get-MemoryStatus $a.Name
    $memLabel = if ($mem.Exists) { "$($mem.Size)($($mem.FileCount))" } else { "-" }
    $lastWrite = if ($mem.Exists) { $mem.LastWrite } else { "-" }
    
    $color = switch ($a.Model) {
        "opus"   { "Red" }
        "sonnet" { "Yellow" }
        "haiku"  { "Green" }
        default  { "White" }
    }
    
    $line = "  {0,-4} {1,-15} {2,-8} {3,-5} {4,-12} {5,-18} {6,-10} {7}" -f $icon, $a.Name, $a.Model, $cost, $access, $mcps, $memLabel, $lastWrite
    Write-Host $line -ForegroundColor $color
}

Write-Host ""

# Detailed mode
if ($Detailed) {
    Write-Host "===============================================================================" -ForegroundColor Cyan
    Write-Host "  Detail View" -ForegroundColor White
    Write-Host "===============================================================================" -ForegroundColor Cyan
    
    foreach ($a in $agents) {
        $icon = Get-ModelIcon $a.Model
        $access = Get-AccessMode $a
        Write-Host ""
        Write-Host "  $icon $($a.Name) [$($a.Model)]" -ForegroundColor White
        Write-Host "    Access: $access | Tools: $($a.Tools)" -ForegroundColor Gray
        if ($a.Skills.Count -gt 0) {
            Write-Host "    Skills: $($a.Skills -join ', ')" -ForegroundColor Magenta
        }
        $desc = $a.Description
        if ($desc.Length -gt 90) { $desc = $desc.Substring(0, 87) + "..." }
        Write-Host "    $desc" -ForegroundColor DarkGray
    }
    Write-Host ""
}

# Memory overview mode
if ($Memory) {
    Write-Host "===============================================================================" -ForegroundColor Cyan
    Write-Host "  Agent Memory Overview" -ForegroundColor White
    Write-Host "===============================================================================" -ForegroundColor Cyan
    
    $hasAnyMemory = $false
    foreach ($a in $agents) {
        $mem = Get-MemoryStatus $a.Name
        if (-not $mem.Exists) { continue }
        $hasAnyMemory = $true
        
        $icon = Get-ModelIcon $a.Model
        Write-Host ""
        Write-Host "  $icon $($a.Name) - $($mem.Size) ($($mem.FileCount) file(s)) - Last: $($mem.LastWrite)" -ForegroundColor White
        
        foreach ($mf in $mem.Files) {
            $relName = $mf.Name
            $fSize = if ($mf.Length -gt 1024) { "{0:N1}KB" -f ($mf.Length / 1024) } else { "{0}B" -f $mf.Length }
            Write-Host ""
            Write-Host "    [$relName] $fSize  $($mf.LastWriteTime.ToString('yyyy-MM-dd HH:mm'))" -ForegroundColor Cyan
            $preview = Get-MemoryPreview $mf.FullName 5
            foreach ($p in $preview) {
                Write-Host "      $p" -ForegroundColor Gray
            }
        }
    }
    if (-not $hasAnyMemory) {
        Write-Host ""
        Write-Host "  No agent has memory data yet." -ForegroundColor DarkGray
    }
    Write-Host ""
}

# Git history mode
if ($History) {
    Write-Host "===============================================================================" -ForegroundColor Cyan
    Write-Host "  Recent Claude Commits (last 15)" -ForegroundColor White
    Write-Host "===============================================================================" -ForegroundColor Cyan
    Write-Host ""
    
    try {
        $commits = git log --oneline -n 15 --grep="Co-Authored-By: Claude" 2>$null
        if (-not $commits) {
            $commits = git log --oneline -n 15 --author="Claude" 2>$null
        }
        if ($commits) {
            foreach ($c in ($commits -split '\n')) {
                if ($c.Trim()) {
                    Write-Host "    $($c.Trim())" -ForegroundColor Gray
                }
            }
        } else {
            Write-Host "    No Claude-authored commits found." -ForegroundColor DarkGray
            Write-Host "    (Looking for 'Co-Authored-By: Claude' in commit messages)" -ForegroundColor DarkGray
        }
    } catch {
        Write-Host "    [ERROR] git not available or not a git repo" -ForegroundColor Red
    }
    
    # Also show per-agent file change stats
    Write-Host ""
    Write-Host "  --- Agent-Related File Changes (last 7 days) ---" -ForegroundColor DarkGray
    
    foreach ($a in $agents) {
        $agentDir = ""
        switch ($a.Name) {
            "rust-impl"     { $agentDir = "crates/" }
            "ts-impl"       { $agentDir = "frontend/src/" }
            "ux-reviewer"   { $agentDir = "frontend/src/components/" }
            "debugger"      { $agentDir = "crates/" }
            "validator"     { $agentDir = "" }
            "lead"          { $agentDir = "" }
            "code-reviewer" { $agentDir = "" }
        }
        
        if ($agentDir) {
            try {
                $count = (git log --oneline --since="7 days ago" -- $agentDir 2>$null | Measure-Object -Line).Lines
                if ($count -gt 0) {
                    Write-Host "    $($a.Name): $count commits in $agentDir" -ForegroundColor Yellow
                }
            } catch {}
        }
    }
    Write-Host ""
}

# Usage hint
Write-Host "  Usage:" -ForegroundColor DarkGray
Write-Host "    .\scripts\show-agents.ps1              # Summary table" -ForegroundColor DarkGray
Write-Host "    .\scripts\show-agents.ps1 -Detailed    # Config details" -ForegroundColor DarkGray
Write-Host "    .\scripts\show-agents.ps1 -Memory      # Memory contents" -ForegroundColor DarkGray
Write-Host "    .\scripts\show-agents.ps1 -History     # Git activity" -ForegroundColor DarkGray
Write-Host "    .\scripts\show-agents.ps1 -Agent lead  # Single agent" -ForegroundColor DarkGray
Write-Host ""
