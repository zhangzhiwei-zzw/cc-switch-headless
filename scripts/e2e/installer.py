#!/usr/bin/env python3
"""Exercise installer upgrades with a local artifact server and fake systemd."""
import functools
import hashlib
import http.server
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import threading

ROOT=Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix='cc-switch-installer-') as temporary:
    root=Path(temporary); artifacts=root/'artifacts';artifacts.mkdir(); mock=root/'mock';mock.mkdir()
    arch={'x86_64':'x86_64','amd64':'x86_64','aarch64':'aarch64','arm64':'aarch64'}[platform.machine()]
    asset=artifacts/f'cc-switch-server-linux-{arch}'
    def publish(content):
        asset.write_bytes(content)
        asset.with_suffix('.sha256').write_text(hashlib.sha256(content).hexdigest()+'  '+asset.name+'\n')
    publish(b'#!/usr/bin/env bash\n[ "${1:-}" = "--help" ]\n')
    systemctl=mock/'systemctl'
    systemctl.write_text('#!/usr/bin/env bash\nprintf "%s\\n" "$*" >> "$CC_SWITCH_TEST_SYSTEMCTL_LOG"\n')
    systemctl.chmod(0o755)
    class Handler(http.server.SimpleHTTPRequestHandler):
        def log_message(self,*args): pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(Handler,directory=str(artifacts)))
    threading.Thread(target=server.serve_forever,daemon=True).start()
    try:
        home=root/'home';home.mkdir();installed=root/'bin with spaces';installed.mkdir()
        binary=installed/'cc-switch-server';binary.write_text('old executable')
        env=dict(os.environ,HOME=str(home),PATH=str(mock)+os.pathsep+os.environ['PATH'],CC_SWITCH_BASE_URL=f'http://127.0.0.1:{server.server_port}',CC_SWITCH_TEST_SYSTEMCTL_LOG=str(root/'systemctl.log'))
        args=['bash',str(ROOT/'scripts/install-server.sh'),'--dir',str(installed),'--service']
        success=subprocess.run(args,env=env,capture_output=True,text=True)
        assert success.returncode==0,success.stdout+success.stderr
        assert binary.read_bytes()==asset.read_bytes()
        assert (installed/'cc-switch-server.previous').read_text()=='old executable'
        calls=(root/'systemctl.log').read_text()
        assert 'restart cc-switch-server' in calls and 'enable cc-switch-server' in calls,calls
        unit=(home/'.config/systemd/user/cc-switch-server.service').read_text()
        assert f'ExecStart="{binary}" --port 15800' in unit
        print('  PASS verified upgrade preserves previous binary and restarts existing service',flush=True)
        current=binary.read_bytes();prior=(installed/'cc-switch-server.previous').read_bytes()
        publish(b'#!/usr/bin/env bash\nexit 1\n')
        rejected=subprocess.run(args,env=env,capture_output=True,text=True)
        assert rejected.returncode != 0
        assert binary.read_bytes()==current and (installed/'cc-switch-server.previous').read_bytes()==prior
        print('  PASS invalid executable never replaces a working installation',flush=True)
        bad_port=subprocess.run(args+['--port','15800;bad'],env=env,capture_output=True,text=True)
        assert bad_port.returncode != 0
        print('  PASS invalid service port rejected',flush=True)
    finally:
        server.shutdown();server.server_close()
