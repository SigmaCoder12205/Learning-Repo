#!/usr/bin/env python3
"""Optional real-Keycloak integration check; Python is only a development test dependency.
Creates one temporary account and deletes it through the admin API in finally.
Never prints passwords, tokens, cookies, or bootstrap secrets.
"""
import argparse
import http.cookiejar
import json
from pathlib import Path
import secrets
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from html.parser import HTMLParser

ROOT = Path(__file__).resolve().parents[2]
BASE = 'http://127.0.0.1:8787'
REALM = BASE + '/realms/cppbench'

class FormParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.forms = []
        self.current = None
    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'form':
            self.current = {'action': attrs.get('action'), 'fields': {}}
            self.forms.append(self.current)
        elif tag == 'input' and self.current is not None and attrs.get('name'):
            self.current['fields'][attrs['name']] = attrs.get('value', '')
    def handle_endtag(self, tag):
        if tag == 'form':
            self.current = None

def request(url, fields=None, bearer=None, method=None):
    headers = {}
    data = None
    if fields is not None:
        data = urllib.parse.urlencode(fields).encode()
        headers['Content-Type'] = 'application/x-www-form-urlencoded'
    if bearer:
        headers['Authorization'] = 'Bearer ' + bearer
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=10) as response:
        body = response.read()
        return json.loads(body) if body else None

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/bench')
    parser.add_argument('--restart', action='store_true')
    args = parser.parse_args()
    config = dict(line.split('=', 1) for line in (ROOT / 'auth/.env').read_text().splitlines() if '=' in line)
    admin = request(BASE + '/realms/master/protocol/openid-connect/token', {
        'client_id': 'admin-cli', 'grant_type': 'password', 'username': 'admin',
        'password': config['KEYCLOAK_ADMIN_PASSWORD']})['access_token']
    username = 'bench-smoke-' + secrets.token_hex(6)
    password = secrets.token_urlsafe(24)
    user_id = None
    try:
        with tempfile.TemporaryDirectory(prefix='bench-keycloak-test-') as cache:
            def cli(*argv, expected=0):
                output = subprocess.run([str(args.binary), 'account', '--data-dir', cache, '--json', *argv], capture_output=True, text=True, timeout=20)
                assert output.returncode == expected, f'Unexpected exit for {argv[0]}: {output.returncode}; {output.stderr}'
                assert password not in output.stdout
                return json.loads(output.stdout)

            def browser_login(action):
                child = subprocess.Popen([str(args.binary), 'account', '--data-dir', cache, '--json', action, username, '--no-browser', '--timeout', '30'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                try:
                    while True:
                        line = child.stderr.readline()
                        assert line, 'CLI exited before browser authorization'
                        if line.startswith('http://'):
                            url = line.strip()
                            break
                    jar = http.cookiejar.CookieJar()
                    browser = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
                    with browser.open(url, timeout=10) as response:
                        html = response.read().decode()
                    # Modern browsers treat loopback as trustworthy for Secure cookies,
                    # whereas Python's cookie jar requires HTTPS even on 127.0.0.1.
                    for cookie in jar:
                        if cookie.domain == '127.0.0.1':
                            cookie.secure = False
                    forms = FormParser(); forms.feed(html)
                    target = next((form for form in forms.forms if 'password' in form['fields']), None)
                    assert target is not None, 'Keycloak did not render a credential form'
                    fields = target['fields']
                    fields.update(username=username, password=password)
                    if action == 'register':
                        fields.update({'email': username + '@example.test', 'firstName': 'CLI', 'lastName': 'Smoke', 'password-confirm': password})
                    # Do not submit alternative submit buttons such as Cancel.
                    fields.pop('cancel', None)
                    form_request = urllib.request.Request(target['action'], urllib.parse.urlencode(fields).encode(), headers={'Origin': BASE, 'Referer': url})
                    try:
                        with browser.open(form_request, timeout=15) as response:
                            response.read()
                    except urllib.error.HTTPError as error:
                        class Messages(HTMLParser):
                            def handle_data(self, data):
                                if any(word in data.lower() for word in ('invalid', 'error', 'required', 'expired')) and len(data) < 300:
                                    print('Keycloak form error:', data.strip())
                        Messages().feed(error.read().decode())
                        raise
                    stdout, stderr = child.communicate(timeout=20)
                    assert child.returncode == 0, f'Browser {action} failed: {stderr}'
                    assert password not in stdout
                    return json.loads(stdout)
                finally:
                    if child.poll() is None:
                        child.kill(); child.communicate()

            result = browser_login('register')
            user_id = result['account']['id']
            assert cli('whoami')['account']['id'] == user_id
            first_refresh = json.loads((Path(cache) / 'session.json').read_text())['session']['refresh_token']
            cli('logout')
            try:
                request(REALM + '/protocol/openid-connect/token', {'grant_type': 'refresh_token', 'client_id': 'cppbench-cli', 'refresh_token': first_refresh})
            except urllib.error.HTTPError as error:
                assert error.code == 400
            else:
                raise AssertionError('Revoked refresh token was still accepted')
            assert browser_login('login')['account']['id'] == user_id
            if args.restart:
                subprocess.run(['docker', 'compose', '-f', str(ROOT / 'auth/compose.yaml'), 'restart', 'keycloak'], check=True, stdout=subprocess.DEVNULL)
                deadline = time.monotonic() + 90
                while True:
                    try:
                        request(REALM + '/.well-known/openid-configuration')
                        break
                    except (OSError, urllib.error.URLError):
                        assert time.monotonic() < deadline, 'Keycloak did not restart'
                        time.sleep(1)
                assert cli('whoami')['account']['id'] == user_id
                print('Persistent login survived Keycloak restart.')
            realm = request(BASE + '/admin/realms/cppbench', bearer=admin)
            assert realm['offlineSessionIdleTimeout'] == 315360000
            assert realm['offlineSessionMaxLifespanEnabled'] is False
            assert realm['registrationAllowed'] is True
            roles = request(BASE + '/admin/realms/cppbench/users/' + user_id + '/role-mappings', bearer=admin)
            assert roles['realmMappings'], 'New account has no default roles'
            clients = request(BASE + '/admin/realms/cppbench/clients?clientId=account', bearer=admin)
            account_roles = request(BASE + '/admin/realms/cppbench/users/' + user_id + '/role-mappings/clients/' + clients[0]['id'] + '/composite', bearer=admin)
            assert 'delete-account' in {role['name'] for role in account_roles}
            actions = request(BASE + '/admin/realms/cppbench/authentication/required-actions', bearer=admin)
            enabled = {action['alias'] for action in actions if action['enabled']}
            assert {'UPDATE_PASSWORD', 'delete_account'} <= enabled
            cli('logout')
            cli('whoami', expected=1)
            print('Real registration, login, whoami, remote logout/revocation, and session settings passed.')
    finally:
        # Find by the unique test name even if registration succeeded but callback handling failed.
        users = request(BASE + '/admin/realms/cppbench/users?username=' + username + '&exact=true', bearer=admin)
        for user in users:
            request(BASE + '/admin/realms/cppbench/users/' + user['id'], bearer=admin, method='DELETE')
        print('Temporary Keycloak test account removed.')

if __name__ == '__main__':
    main()
