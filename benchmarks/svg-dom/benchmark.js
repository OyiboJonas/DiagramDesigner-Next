import {
  BENCHMARK_COUNTS,
  BENCHMARK_MODES,
  benchmarkCaseMatrix,
  runSvgDomCase,
} from '../../apps/desktop/ui/renderer-benchmark-core.mjs';

const svg = document.getElementById('diagram');
const scene = document.getElementById('scene');
const countSelect = document.getElementById('count');
const modeSelect = document.getElementById('mode');
const runButton = document.getElementById('run');
const runAllButton = document.getElementById('runAll');
const status = document.getElementById('status');
const output = document.getElementById('output');

let running = false;

for (const count of BENCHMARK_COUNTS) {
  if (![...countSelect.options].some((option) => Number(option.value) === count)) {
    throw new Error(`benchmark UI is missing node-count option ${count}`);
  }
}
for (const mode of BENCHMARK_MODES) {
  if (![...modeSelect.options].some((option) => option.value === mode)) {
    throw new Error(`benchmark UI is missing mode option ${mode}`);
  }
}

async function runCase(count, mode) {
  if (running) throw new Error('benchmark is already running');
  running = true;
  setControlsDisabled(true);
  status.textContent = `running ${count.toLocaleString()} / ${mode}`;

  try {
    const result = await runSvgDomCase({ count, mode, svg, scene });
    output.textContent += `${JSON.stringify(result, null, 2)}\n`;
    status.textContent = `done ${count.toLocaleString()} / ${mode}`;
    return result;
  } finally {
    running = false;
    setControlsDisabled(false);
  }
}

runButton.addEventListener('click', async () => {
  output.textContent = '';
  try {
    await runCase(Number(countSelect.value), modeSelect.value);
  } catch (error) {
    output.textContent = `${error.stack ?? error}\n`;
    status.textContent = 'failed';
  }
});

runAllButton.addEventListener('click', async () => {
  output.textContent = '';
  setControlsDisabled(true);
  try {
    for (const { count, mode } of benchmarkCaseMatrix()) {
      running = false;
      await runCase(count, mode);
    }
    status.textContent = 'all cases complete';
  } catch (error) {
    output.textContent += `${error.stack ?? error}\n`;
    status.textContent = 'failed';
  } finally {
    running = false;
    setControlsDisabled(false);
  }
});

function setControlsDisabled(disabled) {
  runButton.disabled = disabled;
  runAllButton.disabled = disabled;
  countSelect.disabled = disabled;
  modeSelect.disabled = disabled;
}
