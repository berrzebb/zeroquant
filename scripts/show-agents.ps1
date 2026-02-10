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
    
    $memFile = Join-Path $memoryDir "$AgentName.md"
    if (Test-Path $memFile) {
        $info = Get-Item $memFile
        $sizeVal = $info.Length
        $size = if ($sizeVal -gt 1024) { "{0:N1}KB" -f ($sizeVal / 1024) } else { "{0}B" -f $sizeVal }
        $lastWrite = $info.LastWriteTime.ToString("yyyy-MM-dd HH:mm")
        return @{ Exists = $true; Size = $size; LastWrite = $lastWrite }
    }
    return @{ Exists = $false; Size = "-"; LastWrite = "-" }
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
        Write-Host "  Size:         $($mem.Size)" -ForegroundColor Green
        Write-Host "  Last Updated: $($mem.LastWrite)" -ForegroundColor Green
    } else {
        Write-Host "  No memory file (never used)" -ForegroundColor DarkGray
    }
    Write-Host ""
    Write-Host "  --- Description ---" -ForegroundColor DarkGray
    Write-Host "  $($target.Description)" -ForegroundColor Gray
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
$header = "  {0,-4} {1,-15} {2,-8} {3,-5} {4,-12} {5,-22} {6,-8} {7}" -f "","AGENT","MODEL","COST","ACCESS","MCP SERVERS","MEMORY","LAST ACTIVE"
Write-Host $header -ForegroundColor DarkCyan
Write-Host "  $('-' * 88)" -ForegroundColor DarkGray

foreach ($a in $agents) {
    $icon = Get-ModelIcon $a.Model
    $cost = Get-ModelCostLabel $a.Model
    $mcps = if ($a.McpServers.Count -gt 0) { ($a.McpServers -join ",") } else { "-" }
    $access = Get-AccessMode $a
    $mem = Get-MemoryStatus $a.Name
    $memStatus = if ($mem.Exists) { $mem.Size } else { "-" }
    $lastWrite = if ($mem.Exists) { $mem.LastWrite } else { "-" }
    
    $color = switch ($a.Model) {
        "opus"   { "Red" }
        "sonnet" { "Yellow" }
        "haiku"  { "Green" }
        default  { "White" }
    }
    
    $line = "  {0,-4} {1,-15} {2,-8} {3,-5} {4,-12} {5,-22} {6,-8} {7}" -f $icon, $a.Name, $a.Model, $cost, $access, $mcps, $memStatus, $lastWrite
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

# Usage hint
Write-Host "  Usage:" -ForegroundColor DarkGray
Write-Host "    .\scripts\show-agents.ps1              # Summary table" -ForegroundColor DarkGray
Write-Host "    .\scripts\show-agents.ps1 -Detailed    # With details" -ForegroundColor DarkGray
Write-Host "    .\scripts\show-agents.ps1 -Agent lead  # Single agent" -ForegroundColor DarkGray
Write-Host ""
