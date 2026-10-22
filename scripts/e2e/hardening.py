#!/usr/bin/env python3
"""Isolated regressions for Web filesystem boundaries, sessions and service lifecycle."""
import gzip
import http.client
import http.server
import threading
import json
import os
from pathlib import Path
import socket
import sqlite3
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
BINARY = os.environ.get('CC_SWITCH_SERVER_BIN', str(ROOT / 'src-tauri/target/debug/cc-switch-server'))
checks = 0

def check(condition, message):
    global checks
    assert condition, message
    checks += 1
    print('  PASS', message, flush=True)

with tempfile.TemporaryDirectory(prefix='cc-switch-hardening-') as temporary:
    root = Path(temporary)
    home = root / 'home'; home.mkdir()
    config = home / '.cc-switch'; config.mkdir()
    dist = root / 'web'; dist.mkdir()
    (dist / 'index.html').write_text('CUSTOM_WEB_INDEX')
    (dist / 'large.js').write_text('const value = 1;\n' * 1000)
    (root / 'private.txt').write_text('OUTSIDE_ROOT')
    (dist / 'outside-link').symlink_to(root / 'private.txt')
    (config / 'settings.json').write_text(json.dumps({'sessionAutoSyncEnabled': False}))
    session_id = '11111111-2222-3333-4444-555555555555'
    project = home / '.claude/projects/audit'; project.mkdir(parents=True)
    session = project / (session_id + '.jsonl')
    session.write_text('\n'.join(json.dumps({'sessionId':session_id, 'cwd':'/tmp/audit', 'timestamp':'2026-10-05T00:00:00.000Z', 'type':'user', 'message': {'role':'user','content':f'fixture {i}'}}) for i in range(250)))
    env = {key:value for key,value in os.environ.items() if not key.startswith('CC_SWITCH_')}
    env.update(HOME=str(home), USERPROFILE=str(home), XDG_CONFIG_HOME=str(home/'.config'), CC_SWITCH_CONFIG_DIR=str(config), PI_CODING_AGENT_DIR=str(home/'.pi/agent'))
    with socket.socket() as listener:
        listener.bind(('127.0.0.1',0)); port = listener.getsockname()[1]
    def request(method, path, data=None, token=None, raw=None, headers=None):
        conn = http.client.HTTPConnection('127.0.0.1',port,timeout=5)
        headers = dict(headers or {})
        if token: headers['Authorization'] = 'Bearer ' + token
        if data is not None: raw = json.dumps(data); headers['Content-Type']='application/json'
        conn.request(method,path,raw,headers)
        response=conn.getresponse(); result=(response.status,dict(response.getheaders()),response.read()); conn.close()
        return result
    def invoke(cmd, args=None):
        return json.loads(request('POST','/api/invoke',{'cmd':cmd,'args':args or {}},token)[2])
    with (root/'server.log').open('w') as log:
        proc = subprocess.Popen([BINARY,'--port',str(port),'--dist',str(dist)],env=env,stdout=log,stderr=log)
        try:
            for _ in range(300):
                if proc.poll() is not None: raise RuntimeError((root/'server.log').read_text())
                try:
                    if (config/'web-token').exists() and request('GET','/readyz')[0] == 200: break
                except OSError: pass
                time.sleep(.1)
            else: raise RuntimeError('server readiness timeout')
            token=(config/'web-token').read_text().strip()
            check(request('GET','/healthz')[0] == 200, 'liveness without token')
            check(request('GET','/readyz')[0] == 200, 'database readiness without token')
            check(request('GET','/api/env')[0] == 401, 'diagnostics still require authentication')
            check(request('GET','/../private.txt')[0] == 403, 'unauthenticated parent traversal rejected')
            check(request('GET','/outside-link')[0] == 403, 'escaping symlink rejected')
            check(request('GET','/unknown/route')[2] == b'CUSTOM_WEB_INDEX', 'custom SPA fallback uses custom index')
            compressed=request('GET','/large.js',headers={'Accept-Encoding':'gzip'})
            check(compressed[1].get('content-encoding') == 'gzip' and gzip.decompress(compressed[2]) == (dist/'large.js').read_bytes(), 'static gzip response decodes correctly')
            if os.name != 'nt': check((config/'web-token').stat().st_mode & 0o777 == 0o600, 'token file is private')
            for cmd in ['get_openclaw_live_provider_ids','get_openclaw_default_model','get_hermes_live_provider_ids','get_hermes_model_config']:
                check(invoke(cmd)['ok'], cmd + ' is implemented')
            class Models(http.server.BaseHTTPRequestHandler):
                def do_GET(self):
                    if self.headers.get('Authorization') != 'Bearer fixture-model-key':
                        self.send_error(401); return
                    self.send_response(200); self.send_header('Content-Type','application/json'); self.end_headers()
                    self.wfile.write(b'{"data":[{"id":"fixture-model","owned_by":"fixture"}]}')
                def log_message(self,*args): pass
            models=http.server.ThreadingHTTPServer(('127.0.0.1',0),Models)
            threading.Thread(target=models.serve_forever,daemon=True).start()
            try:
                found=invoke('fetch_models_for_config',{'baseUrl':f'http://127.0.0.1:{models.server_port}','apiKey':'fixture-model-key'})
                check(found['ok'] and found['data'][0]['id']=='fixture-model', 'model discovery bridges shared HTTP service with authentication')
            finally: models.shutdown();models.server_close()
            uploads=[]
            for data in [b'first',b'second']:
                status,_,body=request('POST','/api/upload?name=backup.sql',token=token,raw=data)
                check(status == 200, 'same-name upload accepted')
                uploads.append(Path(json.loads(body)['path']))
            check(uploads[0] != uploads[1] and uploads[0].read_bytes() == b'first' and uploads[1].read_bytes() == b'second', 'concurrent transfers have distinct private paths')
            stale=config/'uploads/stale'; stale.mkdir(); (stale/'old').write_text('expired')
            os.utime(stale,(time.time()-90000,time.time()-90000))
            allocated=invoke('web_allocate_export_path',{'defaultName':'backup.sql'})
            check(allocated['ok'] and not stale.exists(), 'allocation cleans expired transfers')
            status,_,body=request('POST','/api/session-transcript',{'providerId':'claude','sourcePath':str(session)},token)
            chunks=[json.loads(line) for line in body.splitlines()]
            check(status == 200 and chunks[0]['type']=='header' and chunks[-1]['type']=='done', 'NDJSON transcript has header and completion')
            batches=[chunk for chunk in chunks if chunk['type']=='messages']
            check(len(batches)>1 and sum(len(chunk['messages']) for chunk in batches)==250, 'large transcript delivered in multiple batches')
            stream=http.client.HTTPConnection('127.0.0.1',port,timeout=5)
            stream.request('GET','/api/events',headers={'Authorization':'Bearer '+token})
            response=stream.getresponse()
            result=request('POST','/api/rotate-token',{},token)
            new_token=json.loads(result[2])['token']
            check(response.read()==b'', 'rotation closes previously authenticated SSE')
            stream.close()
            check(request('GET','/api/env',token=token)[0]==401, 'rotation rejects old token')
            token=new_token
            check((config/'web-token').read_text()==token, 'rotated token persisted')
            (config/'web-token').unlink(); (config/'web-token').mkdir()
            failed=request('POST','/api/rotate-token',{},token)
            check(failed[0]==500 and json.loads(failed[2])['persisted'] is False, 'failed persistence is reported')
            check(request('GET','/api/env',token=token)[0]==200, 'failed rotation preserves active token')
            (config/'web-token').rmdir(); (config/'web-token').write_text(token)
            sparse=config/'uploads/quota'; sparse.mkdir()
            with (sparse/'large').open('wb') as file: file.truncate(512*1024*1024)
            check(request('POST','/api/upload?name=x.sql',token=token,raw=b'x')[0]==507, 'transfer quota enforced')
            (sparse/'large').unlink(); sparse.rmdir()
            current_log=config/'logs/cc-switch.log'
            with current_log.open('r+b') as file: file.truncate(10*1024*1024)
            request('POST','/api/upload?name=rotation.sql',token=token,raw=b'log trigger')
            check((config/'logs/cc-switch.log.1').exists(), 'application log rotates at 10 MiB')
            proc.terminate(); proc.wait(timeout=15)
            check(proc.returncode==0, 'SIGTERM drains service and exits successfully')
            with sqlite3.connect(config/'cc-switch.db') as database: database.execute('PRAGMA user_version=99999')
            proc=subprocess.Popen([BINARY,'--port',str(port),'--dist',str(dist)],env=env,stdout=log,stderr=log)
            for _ in range(100):
                try:
                    if request('GET','/healthz')[0]==200: break
                except OSError: pass
                time.sleep(.1)
            check(request('GET','/readyz')[0]==503, 'newer database fails readiness without killing recovery UI')
            check(invoke('get_init_error')['ok'], 'initialization failure remains inspectable')
            proc.terminate();proc.wait(timeout=15)
        finally:
            if proc.poll() is None:
                proc.kill();proc.wait()
print(f'All {checks} Web hardening regressions passed.')
