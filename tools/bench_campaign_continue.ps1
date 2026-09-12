[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Campaign,
    [Parameter(Mandatory = $true)][string]$Staging,
    [Parameter(Mandatory = $true)][string]$Image,
    [Parameter(Mandatory = $true)][int]$InitialGeneratorId,
    [switch]$ConfirmSubscriptionUsage
)

# Host-side orchestration only. The frozen Python helpers own all decisions
# about prompt identity, completed work, attempt evidence, grading, and repair.
$ErrorActionPreference = 'Stop'
if (-not $ConfirmSubscriptionUsage) {
    throw 'Explicit -ConfirmSubscriptionUsage is required before any repair calls.'
}
if ($InitialGeneratorId -le 0) {
    throw 'Supply the observed initial generator process ID.'
}

function Invoke-BenchmarkStep {
    param([string[]]$StepArguments)
    & python @StepArguments
    if ($LASTEXITCODE -ne 0) {
        throw "Benchmark step failed ($LASTEXITCODE): $($StepArguments -join ' ')"
    }
}

try {
    $taskGenerator = Get-Process -Id $InitialGeneratorId -ErrorAction SilentlyContinue
    if ($null -ne $taskGenerator) {
        $taskGeneratorInfo = Get-CimInstance Win32_Process -Filter "ProcessId = $InitialGeneratorId"
        if ($null -ne $taskGeneratorInfo -and (
            $taskGeneratorInfo.Name -notmatch '^python' -or
            -not $taskGeneratorInfo.CommandLine.Contains('jett_bench_campaign.py') -or
            -not $taskGeneratorInfo.CommandLine.Contains('generate') -or
            -not $taskGeneratorInfo.CommandLine.Contains($Campaign)
        )) {
            throw 'Observed PID does not identify the expected campaign generator.'
        }
    }
    else {
        Write-Output 'Initial generator PID is absent; full-generation checks must still pass before repair.'
    }

    # A prior supervisor may have stopped midway through a staging batch.
    # Finish that exact snapshot before extending it; completed grades are
    # validated and retained by the frozen grader, never selectively retried.
    Invoke-BenchmarkStep -StepArguments @(
        'tools/jett_bench_campaign.py', 'grade', $Staging, '--image', $Image, '--jobs', '3'
    )

    $taskLastLength = -1L
    while ($true) {
        $taskExited = $null -eq $taskGenerator -or $taskGenerator.HasExited
        $taskRawLength = (Get-Item -LiteralPath (Join-Path $Campaign 'initial-raw.jsonl')).Length
        if ($taskRawLength -ne $taskLastLength -or $taskExited) {
            # Snapshot rejects an incomplete concurrent JSONL append. Fail
            # closed on any error; an operator may restart this supervisor
            # after inspection, without restarting the initial generator.
            Invoke-BenchmarkStep -StepArguments @(
                'tools/bench_campaign_staging.py', 'snapshot', $Campaign, $Staging, '--stage', 'initial'
            )
            Invoke-BenchmarkStep -StepArguments @(
                'tools/jett_bench_campaign.py', 'grade', $Staging, '--image', $Image, '--jobs', '3'
            )
            $taskLastLength = $taskRawLength
        }
        if ($taskExited) {
            break
        }
        # Hold the actual process handle; a timeout is not completion and never
        # causes a generator restart. Recheck and take a final snapshot on exit.
        [void]$taskGenerator.WaitForExit(15000)
    }

    Invoke-BenchmarkStep -StepArguments @(
        'tools/bench_campaign_staging.py', 'merge', $Campaign, $Staging, '--stage', 'initial'
    )
    Invoke-BenchmarkStep -StepArguments @(
        'tools/jett_bench_campaign.py', 'grade', $Campaign, '--image', $Image, '--jobs', '3'
    )
    Write-Output 'Complete initial grading verified; beginning failed-only paired repair.'
    Invoke-BenchmarkStep -StepArguments @(
        'tools/jett_bench_campaign.py', 'generate', $Campaign, '--stage', 'repair', '--jobs', '3',
        '--confirm-subscription-usage'
    )
    Invoke-BenchmarkStep -StepArguments @(
        'tools/jett_bench_campaign.py', 'grade', $Campaign, '--stage', 'repair', '--image', $Image, '--jobs', '3'
    )
    Invoke-BenchmarkStep -StepArguments @('tools/jett_bench_campaign.py', 'report', $Campaign)
    Write-Output 'Original campaign complete. Skill revision and its separate full-task follow-up are still required.'
}
finally {
    if ($null -ne $taskGenerator) {
        $taskGenerator.Dispose()
    }
}
