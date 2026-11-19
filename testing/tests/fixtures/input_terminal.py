"""Drive bench through a real terminal; used by the Rust integration tests."""
import json
import os
import pty
import select
import signal
import subprocess
import sys
import time

binary, workload, mode = sys.argv[1:4]
master, slave = pty.openpty()
args = [binary, workload, '--runs=2', '--warmup=1', '--format=json',
        '--interactive-timeout=0.15' if mode == 'timeout' else '--interactive-timeout=5']
if mode != 'auto':
    args.append('--interactive')
if mode == 'explore':
    args += ['--explore-inputs', '--max-cases=4']
if mode == 'output_limit':
    args += ['--output-limit=0.01']
child = subprocess.Popen(args, stdin=slave, stdout=subprocess.PIPE,
                         stderr=slave, start_new_session=True)
os.close(slave)
transcript = bytearray()
stdout = bytearray()
step = 0
deadline = time.monotonic() + 10
fds = [master, child.stdout.fileno()]
try:
    while fds and time.monotonic() < deadline:
        ready, _, _ = select.select(fds, [], [], 0.1)
        for fd in ready:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                chunk = b''
            if not chunk:
                fds.remove(fd)
                continue
            if fd == master:
                transcript.extend(chunk)
            else:
                stdout.extend(chunk)
        if step == 0 and b'LIVE1>' in transcript:
            if mode == 'cancel':
                os.kill(child.pid, signal.SIGINT)
            elif mode != 'timeout':
                os.write(master, b'7\n')
            step = 1
        if step == 1 and b'LIVE2>' in transcript and mode not in ('cancel', 'timeout'):
            os.write(master, b'2.5\n')
            step = 2
        if mode == 'eof' and step == 0 and b'EOF>' in transcript:
            os.write(master, b'\x04')
            step = 2
        if child.poll() is not None and not ready:
            break
    # Pipe/PTY EOF can precede the parent's waitable exit by a few scheduler ticks.
    if not fds:
        child.wait(timeout=max(0.1, deadline - time.monotonic()))
    if child.poll() is None:
        os.kill(child.pid, signal.SIGINT)
        child.wait(timeout=3)
        raise RuntimeError('interactive test timed out: ' + transcript.decode(errors='replace'))
    # read any remaining report bytes after process exit
    stdout.extend(child.stdout.read())
    print(json.dumps({'code': child.returncode, 'report': json.loads(stdout),
                      'terminal': transcript.decode(errors='replace')}))
finally:
    if child.poll() is None:
        child.kill()
        child.wait()
    os.close(master)
