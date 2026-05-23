#!/usr/bin/env node
import { spawn } from "node:child_process";
import { cp, mkdir, mkdtemp } from "node:fs/promises";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../..");

function parseArgs(argv) {
  const args = { workDir: null };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--work-dir") {
      index += 1;
      if (index >= argv.length) {
        throw new Error("--work-dir requires a value");
      }
      args.workDir = resolve(argv[index]);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return args;
}

async function copyFixture(workDir) {
  const sourceDir = join(repoRoot, "examples");
  for (const name of [
    "json-repair.yaml",
    "question.txt",
    "prompt.tmpl",
    "policy.md",
    "answer.schema.json",
  ]) {
    await cp(join(sourceDir, name), join(workDir, name));
  }
  return join(workDir, "json-repair.yaml");
}

function runProcess(command, args, options = {}) {
  return new Promise((resolveProcess) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", (error) => {
      stderr += `${error.message}\n`;
      resolveProcess({ code: 1, stdout, stderr });
    });
    child.on("close", (code) => {
      resolveProcess({ code: code ?? 1, stdout, stderr });
    });
  });
}

function parseJsonLines(output) {
  return output
    .split(/\r?\n/)
    .filter((line) => line.trim().startsWith("{"))
    .map((line) => JSON.parse(line));
}

function resolveCommand(command) {
  if (!command.includes("/") && !command.includes("\\")) {
    return command;
  }
  return isAbsolute(command) ? command : resolve(process.cwd(), command);
}

async function runPipeline(workDir) {
  await mkdir(workDir, { recursive: true });
  const manifest = await copyFixture(workDir);
  const trace = join(workDir, "trace.jsonl");
  const checkpoint = join(workDir, "checkpoint.json");
  const llmff = resolveCommand(process.env.LLMFF_BIN ?? "llmff");
  const env = {
    ...process.env,
    LLMFF_MOCK_BAD_RESPONSE:
      process.env.LLMFF_MOCK_BAD_RESPONSE ?? '{"wrong":true}',
    LLMFF_MOCK_GOOD_RESPONSE:
      process.env.LLMFF_MOCK_GOOD_RESPONSE ?? '{"answer":"ok"}',
  };

  const inspect = await runProcess(
    llmff,
    ["inspect", manifest, "--format", "json"],
    { cwd: workDir, env },
  );
  if (inspect.code !== 0) {
    if (inspect.stderr) {
      process.stderr.write(inspect.stderr);
    }
    return inspect.code;
  }

  const report = JSON.parse(inspect.stdout);
  console.log(`inspect_format_version=${report.format_version}`);
  console.log(`manifest_hash=${report.manifest.hash}`);
  console.log(
    `stdout_manifest_outputs=${report.execution.stdout.manifest_outputs}`,
  );

  const completed = await runProcess(
    llmff,
    [
      "run",
      manifest,
      "--events",
      "-",
      "--trace",
      trace,
      "--checkpoint",
      checkpoint,
      "--timeout-ms",
      "30000",
    ],
    { cwd: workDir, env },
  );

  const events = parseJsonLines(completed.stdout);
  const failures = events.filter((event) => event.event === "run_failed");
  console.log(`run_status=${completed.code === 0 ? "ok" : "failed"}`);
  console.log(`event_count=${events.length}`);
  console.log(`trace=${trace}`);
  console.log(`checkpoint=${checkpoint}`);
  const output = join(workDir, "answer.json");
  console.log(`output=${output}`);
  console.log(`output_exists=${existsSync(output)}`);

  if (failures.length > 0) {
    const failure = failures[failures.length - 1];
    console.error(
      `failure_kind=${failure.failure_kind ?? "unknown"} failure_message=${failure.failure_message ?? ""}`,
    );
  }
  if (completed.stderr) {
    process.stderr.write(completed.stderr);
  }

  return completed.code;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const workDir =
    args.workDir ?? (await mkdtemp(join(tmpdir(), "llmff-agent-node-")));
  return runPipeline(workDir);
}

main()
  .then((code) => {
    process.exitCode = code;
  })
  .catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
