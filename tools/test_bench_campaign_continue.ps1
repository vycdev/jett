# Run with Windows PowerShell 5.1; no Pester, benchmark, grader, or model calls.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script:SupervisorPath = Join-Path $PSScriptRoot 'bench_campaign_continue.ps1'
$script:CampaignPath = 'C:\mock benchmark\canonical'
$script:StagingPath = 'C:\mock benchmark\staging'
$script:Image = 'sha256:' + ('a' * 64)
$script:GeneratorId = 12345
$script:TestCount = 0

function Assert-Equal {
    param($Actual, $Expected, [string]$Message)
    if ($Actual -cne $Expected) {
        throw "${Message}: expected [$Expected], got [$Actual]"
    }
}

function Assert-Steps {
    param([System.Collections.Generic.List[string[]]]$Expected)
    Assert-Equal $script:Calls.Count $Expected.Count 'Subprocess count'
    for ($step = 0; $step -lt $Expected.Count; $step++) {
        Assert-Equal $script:Calls[$step].Length $Expected[$step].Length "Argument count at step $step"
        for ($argument = 0; $argument -lt $Expected[$step].Length; $argument++) {
            Assert-Equal $script:Calls[$step][$argument] $Expected[$step][$argument] "Step $step argument $argument"
        }
    }
}

# These functions deliberately shadow all external commands used by the supervisor.
function python {
    [string[]]$commandArguments = @($args)
    $global:BenchContinueTestState.Calls.Add($commandArguments)
    $global:LASTEXITCODE = 0
    if ($global:BenchContinueTestState.Calls.Count -eq $global:BenchContinueTestState.FailAt) {
        $global:LASTEXITCODE = 19
    }
}

function Get-Process {
    [CmdletBinding()]
    param([int]$Id)
    $global:BenchContinueTestState.ProcessQueries.Add($Id)
    return $global:BenchContinueTestState.ObservedProcess
}

function Get-CimInstance {
    [CmdletBinding()]
    param([Parameter(Position = 0)][string]$ClassName, [string]$Filter)
    Assert-Equal $ClassName 'Win32_Process' 'CIM class'
    Assert-Equal $Filter "ProcessId = $($global:BenchContinueTestState.GeneratorId)" 'CIM filter'
    $global:BenchContinueTestState.CimQueries++
    return $global:BenchContinueTestState.ObservedProcessInfo
}

function Get-Item {
    [CmdletBinding()]
    param([string]$LiteralPath)
    Assert-Equal $LiteralPath (Join-Path $global:BenchContinueTestState.CampaignPath 'initial-raw.jsonl') 'Raw journal path'
    $index = [Math]::Min($global:BenchContinueTestState.RawReads, $global:BenchContinueTestState.RawLengths.Length - 1)
    $global:BenchContinueTestState.RawReads++
    return [pscustomobject]@{ Length = $global:BenchContinueTestState.RawLengths[$index] }
}

function Invoke-Scenario {
    param(
        [ValidateSet('absent', 'live', 'exited')][string]$ProcessMode = 'absent',
        [int]$FailAt = 0,
        [int]$ExitAfterWait = 1,
        [long[]]$RawLengths = @(100),
        [string]$ProcessName = 'python.exe',
        [string]$CommandLine = "python tools/jett_bench_campaign.py generate $script:CampaignPath",
        [int]$RequestedId = $script:GeneratorId,
        [switch]$OmitUsageConfirmation
    )
    $script:Calls = [System.Collections.Generic.List[string[]]]::new()
    $script:ProcessQueries = [System.Collections.Generic.List[int]]::new()
    $script:CimQueries = 0
    $script:FailAt = $FailAt
    $script:RawLengths = $RawLengths
    $script:RawReads = 0
    $script:Failure = $null
    $script:ObservedProcess = $null
    $script:ObservedProcessInfo = [pscustomobject]@{ Name = $ProcessName; CommandLine = $CommandLine }
    if ($ProcessMode -ne 'absent') {
        $script:ObservedProcess = [pscustomobject]@{
            HasExited = $ProcessMode -eq 'exited'
            WaitCalls = 0
            ExitAfterWait = $ExitAfterWait
            DisposeCalls = 0
        }
        $script:ObservedProcess | Add-Member -MemberType ScriptMethod -Name WaitForExit -Value {
            param([int]$Milliseconds)
            Assert-Equal $Milliseconds 15000 'Bounded process wait'
            $this.WaitCalls++
            if ($this.WaitCalls -ge $this.ExitAfterWait) {
                $this.HasExited = $true
            }
            return $this.HasExited
        }
        $script:ObservedProcess | Add-Member -MemberType ScriptMethod -Name Dispose -Value {
            $this.DisposeCalls++
        }
    }
    $parameters = @{
        Campaign = $script:CampaignPath
        Staging = $script:StagingPath
        Image = $script:Image
        InitialGeneratorId = $RequestedId
        ConfirmSubscriptionUsage = -not $OmitUsageConfirmation
    }
    # A child script has its own script scope; mocked commands share only this
    # task-named state, never the host process table or a real campaign path.
    $priorExitCode = Get-Variable -Name LASTEXITCODE -Scope Global -ErrorAction SilentlyContinue
    $global:BenchContinueTestState = [pscustomobject]@{
        Calls = $script:Calls
        ProcessQueries = $script:ProcessQueries
        ObservedProcess = $script:ObservedProcess
        ObservedProcessInfo = $script:ObservedProcessInfo
        FailAt = $script:FailAt
        GeneratorId = $script:GeneratorId
        CampaignPath = $script:CampaignPath
        RawLengths = $script:RawLengths
        RawReads = 0
        CimQueries = 0
    }
    try {
        & $script:SupervisorPath @parameters | Out-Null
    }
    catch {
        $script:Failure = $_.Exception.Message
    }
    finally {
        $script:RawReads = $global:BenchContinueTestState.RawReads
        $script:CimQueries = $global:BenchContinueTestState.CimQueries
        Remove-Variable -Name BenchContinueTestState -Scope Global
        if ($null -eq $priorExitCode) {
            Remove-Variable -Name LASTEXITCODE -Scope Global -ErrorAction SilentlyContinue
        }
        else {
            Set-Variable -Name LASTEXITCODE -Scope Global -Value $priorExitCode.Value
        }
    }
}

function Expected-Steps {
    param([int]$Snapshots = 1)
    $steps = [System.Collections.Generic.List[string[]]]::new()
    $steps.Add(@('tools/jett_bench_campaign.py', 'grade', $script:StagingPath,
                 '--image', $script:Image, '--jobs', '3'))
    for ($index = 0; $index -lt $Snapshots; $index++) {
        $steps.Add(@('tools/bench_campaign_staging.py', 'snapshot', $script:CampaignPath,
                     $script:StagingPath, '--stage', 'initial'))
        $steps.Add(@('tools/jett_bench_campaign.py', 'grade', $script:StagingPath,
                     '--image', $script:Image, '--jobs', '3'))
    }
    $steps.Add(@('tools/bench_campaign_staging.py', 'merge', $script:CampaignPath,
                 $script:StagingPath, '--stage', 'initial'))
    $steps.Add(@('tools/jett_bench_campaign.py', 'grade', $script:CampaignPath,
                 '--image', $script:Image, '--jobs', '3'))
    $steps.Add(@('tools/jett_bench_campaign.py', 'generate', $script:CampaignPath, '--stage', 'repair',
                 '--jobs', '3', '--confirm-subscription-usage'))
    $steps.Add(@('tools/jett_bench_campaign.py', 'grade', $script:CampaignPath, '--stage', 'repair',
                 '--image', $script:Image, '--jobs', '3'))
    $steps.Add(@('tools/jett_bench_campaign.py', 'report', $script:CampaignPath))
    return ,$steps
}

function Run-Test {
    param([string]$Name, [scriptblock]$Body)
    try {
        & $Body
    }
    catch {
        throw "FAIL ${Name}: $($_.Exception.Message)"
    }
    $script:TestCount++
    Write-Output "PASS $Name"
}

Run-Test 'absent PID resumes staging, then follows complete guarded step order' {
    Invoke-Scenario
    Assert-Equal $script:Failure $null 'Successful orchestration'
    Assert-Steps (Expected-Steps)
    Assert-Equal $script:ProcessQueries.Count 1 'Process query count'
    Assert-Equal $script:ProcessQueries[0] $script:GeneratorId 'Observed process ID'
    Assert-Equal $script:CimQueries 0 'Absent PID must not issue CIM query'
}

Run-Test 'existing staging grade failure prevents the first snapshot' {
    Invoke-Scenario -FailAt 1
    Assert-Equal ($script:Failure -like 'Benchmark step failed (19):*') $true 'Grade failure surfaced'
    Assert-Steps ((Expected-Steps).GetRange(0, 1))
    Assert-Equal $script:RawReads 0 'No snapshot loop after failed resume'
}

$expectedAbsent = Expected-Steps
for ($failureIndex = 1; $failureIndex -le $expectedAbsent.Count; $failureIndex++) {
    Run-Test "absent PID stops exactly at failed subprocess $failureIndex" {
        Invoke-Scenario -FailAt $failureIndex
        Assert-Equal ($script:Failure -like 'Benchmark step failed (19):*') $true 'Subprocess failure surfaced'
        Assert-Steps ($expectedAbsent.GetRange(0, $failureIndex))
    }
}

Run-Test 'absent PID cannot reach repair when full-generation merge guard fails' {
    Invoke-Scenario -FailAt 4
    Assert-Equal ($script:Failure -like '*bench_campaign_staging.py merge*') $true 'Merge guard surfaced'
    Assert-Steps ((Expected-Steps).GetRange(0, 4))
    Assert-Equal @($script:Calls | Where-Object { $_[1] -eq 'generate' }).Count 0 'No model calls'
}

Run-Test 'live process takes final snapshot even without journal growth' {
    Invoke-Scenario -ProcessMode live -ExitAfterWait 2
    Assert-Equal $script:Failure $null 'Successful live orchestration'
    Assert-Steps (Expected-Steps -Snapshots 2)
    Assert-Equal $script:ObservedProcess.WaitCalls 2 'Timeout does not mean completion'
    Assert-Equal $script:ObservedProcess.DisposeCalls 1 'Process handle disposed'
    Assert-Equal $script:RawReads 3 'One final observation after process exit'
    Assert-Equal $script:CimQueries 1 'Live process identity inspected'
}

Run-Test 'journal growth triggers incremental grading before final snapshot' {
    Invoke-Scenario -ProcessMode live -ExitAfterWait 2 -RawLengths @(100, 200, 200)
    Assert-Equal $script:Failure $null 'Successful growing-journal orchestration'
    Assert-Steps (Expected-Steps -Snapshots 3)
}

Run-Test 'already-exited process still performs final snapshot and guarded merge' {
    Invoke-Scenario -ProcessMode exited
    Assert-Equal $script:Failure $null 'Successful exited-process orchestration'
    Assert-Steps (Expected-Steps)
    Assert-Equal $script:ObservedProcess.WaitCalls 0 'No wait after process exit'
    Assert-Equal $script:ObservedProcess.DisposeCalls 1 'Exited process disposed'
}

$expectedLive = Expected-Steps -Snapshots 2
for ($failureIndex = 1; $failureIndex -le $expectedLive.Count; $failureIndex++) {
    Run-Test "live PID stops exactly at failed subprocess $failureIndex and disposes handle" {
        Invoke-Scenario -ProcessMode live -FailAt $failureIndex
        Assert-Equal ($script:Failure -like 'Benchmark step failed (19):*') $true 'Subprocess failure surfaced'
        Assert-Steps ($expectedLive.GetRange(0, $failureIndex))
        Assert-Equal $script:ObservedProcess.DisposeCalls 1 'Process disposed after failure'
    }
}

$wrongIdentities = @(
    @{ ProcessName = 'powershell.exe' },
    @{ CommandLine = "python unrelated.py generate $script:CampaignPath" },
    @{ CommandLine = "python tools/jett_bench_campaign.py grade $script:CampaignPath" },
    @{ CommandLine = 'python tools/jett_bench_campaign.py generate C:\other-campaign' }
)
for ($identityIndex = 0; $identityIndex -lt $wrongIdentities.Length; $identityIndex++) {
    Run-Test "wrong generator identity $identityIndex is rejected before any subprocess" {
        $identity = $wrongIdentities[$identityIndex]
        Invoke-Scenario -ProcessMode live @identity
        Assert-Equal $script:Failure 'Observed PID does not identify the expected campaign generator.' 'PID rejection'
        Assert-Equal $script:Calls.Count 0 'No benchmark commands for wrong PID'
        Assert-Equal $script:ObservedProcess.DisposeCalls 1 'Wrong process object disposed'
    }
}

Run-Test 'subscription confirmation is required before process inspection or subprocesses' {
    Invoke-Scenario -OmitUsageConfirmation
    Assert-Equal $script:Failure 'Explicit -ConfirmSubscriptionUsage is required before any repair calls.' 'Usage gate'
    Assert-Equal $script:Calls.Count 0 'No benchmark commands without consent'
    Assert-Equal $script:ProcessQueries.Count 0 'No process inspection without consent'
}

Run-Test 'nonpositive generator ID is rejected before process inspection' {
    Invoke-Scenario -RequestedId 0
    Assert-Equal $script:Failure 'Supply the observed initial generator process ID.' 'PID input gate'
    Assert-Equal $script:Calls.Count 0 'No benchmark commands with invalid PID'
    Assert-Equal $script:ProcessQueries.Count 0 'No process query with invalid PID'
}

Write-Output "All $script:TestCount isolated continuation control-flow tests passed."
