"""Reproduce Zola's transient HTTP 404 during rebuilds on an isolated site copy.

The source article stays valid: only a fixed-width body marker changes in place.
This probes the upstream server, not the browser's retry/recovery behavior.
"""
import argparse
from collections import Counter
import http.client
import json
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import threading
import time


ROOT = Path(__file__).resolve().parents[1]


def probe_rebuilds(writes=40, interval_ms=20, debounce_ms=1):
    scratch = (ROOT / '.tools').resolve()
    scratch.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='rebuild-probe-', dir=scratch) as temporary:
        directory = Path(temporary).resolve()
        if directory.parent != scratch:
            raise RuntimeError('Probe directory escaped the project scratch directory')
        site = directory / 'site'
        site.mkdir()
        for name in ('content', 'templates', 'static', 'sass'):
            source = ROOT / 'site' / name
            if source.exists():
                shutil.copytree(source, site / name)
        shutil.copy2(ROOT / 'site/config.toml', site / 'config.toml')
        article = site / 'content/posts/rebuild-probe/index.md'
        article.parent.mkdir()
        header = b'+++\ntitle = "Rebuild probe"\ndate = 2026-09-12\ndraft = true\n+++\n\n'
        article.write_bytes(header + b'probe-version-00000\n')
        with socket.socket() as candidate:
            candidate.bind(('127.0.0.1', 0))
            port = candidate.getsockname()[1]
        command = [str(ROOT / '.tools/zola/zola.exe'), '--root', str(site),
                   'serve', '--drafts', '--port', str(port),
                   '--debounce', str(debounce_ms)]
        statuses = Counter()
        missing_kinds = Counter()
        writer_errors = []
        finished = threading.Event()
        cancelled = threading.Event()
        connection = None
        worker = None
        process = None

        def page():
            nonlocal connection
            if connection is None:
                connection = http.client.HTTPConnection('127.0.0.1', port, timeout=2)
            try:
                connection.request('GET', '/posts/rebuild-probe/')
                response = connection.getresponse()
                return response.status, response.getheader('Content-Type'), response.read().decode('utf-8')
            except (OSError, http.client.HTTPException):
                connection.close()
                connection = None
                return 0, '', ''

        def write_valid_bodies():
            try:
                for number in range(1, writes + 1):
                    if cancelled.is_set():
                        break
                    # Never truncate or touch frontmatter; every intermediate file is valid.
                    with article.open('r+b') as output:
                        output.seek(len(header))
                        output.write(f'probe-version-{number:05d}\n'.encode('ascii'))
                        output.flush()
                    cancelled.wait(interval_ms / 1000)
            except Exception as error:
                writer_errors.append(repr(error))
            finally:
                finished.set()

        with (directory / 'zola.log').open('w', encoding='utf-8') as log:
            try:
                process = subprocess.Popen(command, stdout=log, stderr=log)
                deadline = time.monotonic() + 20
                while 'probe-version-00000' not in page()[2]:
                    if process.poll() is not None or time.monotonic() >= deadline:
                        raise RuntimeError('Probe server failed to start')
                    time.sleep(.02)
                started = time.monotonic()
                worker = threading.Thread(target=write_valid_bodies)
                worker.start()
                last_marker = f'probe-version-{writes:05d}'
                settled_at = None
                deadline = started + 30
                while time.monotonic() < deadline:
                    status, content_type, body = page()
                    statuses[status] += 1
                    if status == 404:
                        missing_kinds['plain_without_livereload' if body == 'Not Found'
                                      else 'html_with_livereload' if 'livereload.js' in body
                                      else content_type or 'unknown'] += 1
                    if finished.is_set() and status == 200 and last_marker in body:
                        settled_at = settled_at or time.monotonic()
                        if time.monotonic() - settled_at >= .25:
                            break
                    else:
                        settled_at = None
                    time.sleep(.001)
                final_status, _, final_body = page()
                if final_status != 200 or last_marker not in final_body or writer_errors:
                    raise AssertionError({'final_status': final_status, 'writer_errors': writer_errors,
                                          'last_marker_visible': last_marker in final_body})
                return {'writes': writes, 'write_interval_ms': interval_ms, 'debounce_ms': debounce_ms,
                        'duration_ms': round((time.monotonic() - started) * 1000, 1),
                        'http_status_counts': dict(statuses), 'http_404_kinds': dict(missing_kinds),
                        'reproduced_transient_404': statuses[404] > 0,
                        'final_status': final_status, 'final_marker_visible': True}
            finally:
                cancelled.set()
                if worker is not None:
                    worker.join(timeout=5)
                if connection is not None:
                    connection.close()
                if process is not None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--writes', type=int, default=40)
    parser.add_argument('--interval-ms', type=int, default=20)
    parser.add_argument('--debounce-ms', type=int, default=1)
    args = parser.parse_args()
    if not 1 <= args.writes <= 1000 or not 1 <= args.interval_ms <= 1000 or args.debounce_ms < 1:
        parser.error('Use 1-1000 writes, 1-1000 ms intervals, and debounce >= 1 ms')
    print(json.dumps(probe_rebuilds(args.writes, args.interval_ms, args.debounce_ms), indent=2))
