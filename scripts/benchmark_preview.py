"""Measure write-to-served-HTML latency on an isolated copy of the real blog."""
from pathlib import Path
import http.client
import json
import math
import shutil
import socket
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
ZOLA = ROOT / '.tools/zola/zola.exe'

def measure(debounce, fast=False, samples=20):
    with tempfile.TemporaryDirectory(prefix='preview-bench-', dir=ROOT / '.tools') as temporary:
        directory = Path(temporary)
        site = directory / 'site'
        site.mkdir()
        for name in ['content', 'templates', 'static', 'sass']:
            if (ROOT / 'site' / name).exists():
                shutil.copytree(ROOT / 'site' / name, site / name)
        shutil.copy(ROOT / 'site/config.toml', site / 'config.toml')
        article = site / 'content/posts/latency-probe/index.md'
        article.parent.mkdir()
        header = '+++\ntitle = "Latency probe"\ndate = 2026-09-12\ndraft = true\n+++\n\n'
        article.write_text(header + 'probe-initial', encoding='utf-8')
        with socket.socket() as probe:
            probe.bind(('127.0.0.1', 0))
            port = probe.getsockname()[1]
        command = [str(ZOLA), '--root', str(site), 'serve', '--drafts', '--port', str(port), '--debounce', str(debounce)]
        if fast:
            command.append('--fast')
        process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        connection = None
        try:
            def page():
                nonlocal connection
                if connection is None:
                    connection = http.client.HTTPConnection('127.0.0.1', port, timeout=2)
                try:
                    connection.request('GET', '/posts/latency-probe/')
                    response = connection.getresponse()
                    return response.status, response.read().decode('utf-8')
                except (OSError, http.client.HTTPException):
                    connection.close()
                    connection = None
                    return 0, ''
            deadline = time.monotonic() + 20
            while 'probe-initial' not in page()[1]:
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('Benchmark server failed to start')
                time.sleep(.02)
            times = []
            for number in range(samples + 2):
                marker = f'probe-version-{number:03d}'
                start = time.perf_counter()
                article.write_text(header + marker, encoding='utf-8')
                while marker not in page()[1]:
                    if time.perf_counter() - start > 5:
                        raise RuntimeError('Rebuild timeout')
                    time.sleep(.001)
                elapsed = (time.perf_counter() - start) * 1000
                if number >= 2:
                    times.append(elapsed)
                time.sleep(.04)
            return {'debounce_ms': debounce, 'fast': fast, 'samples': samples,
                    'median_ms': round(statistics.median(times), 2),
                    'p95_ms': round(sorted(times)[math.ceil(.95 * len(times)) - 1], 2),
                    'max_ms': round(max(times), 2)}
        finally:
            if connection:
                connection.close()
            process.terminate()
            process.wait(timeout=10)

if __name__ == '__main__':
    results = []
    path = ROOT / '.tools/preview-benchmark.json'
    for debounce, fast in [(200, False), (1, False), (1, True)]:
        try:
            result = measure(debounce, fast)
        except RuntimeError as error:
            result = {'debounce_ms': debounce, 'fast': fast, 'error': str(error)}
        results.append(result)
        path.write_text(json.dumps(results, indent=2), encoding='utf-8')
        print(json.dumps(result), flush=True)
