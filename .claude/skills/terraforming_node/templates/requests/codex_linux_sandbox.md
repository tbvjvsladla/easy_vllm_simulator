# Codex Linux 실행환경 설치 수행지시서

> 추적 PII-free 템플릿. 메인 terraforming_node가 `docs/request/`에 새 시각 이름으로 발행한다.
> `{{...}}`는 발행 시 채우고 이 템플릿 안내는 제거한다. WORKSPACE_SHELL은 shlex.quote로 치환한다.
> target은 서브의 배달된 Card에서 관측하고 메인이 자기 manifest와 대조한다. 운영자 endpoint를 본문에 박지 않는다.
> 이 템플릿은 Ubuntu 24.04·system bwrap·AppArmor userns 거부가 확인된 **서브**에 한정한다.

발행: {{GENERATED_UTC}}. 주관: terraforming_node §2.5.2. 진단 근거: {{DIAGNOSIS}}

## 전제와 변경 범위

- 서브의 일반 사용자 터미널에서 수행한다. 전체 블록을 sudo로 감싸지 않는다. 인증 재로그인은 이미 유효하면 반복하지 않는다.
- 대상 workspace는 manifest의 `nodes[sub].work_dir`에서 해소했다. 현재 `vllm-sub` 프로젝트 설정과 서브 manifest를 먼저 확인한다.
- 먼저 같은 named sandbox에서 `/usr/bin/true`를 확인한다. 정상이라면 **설치를 건너뛴다**.
- 실패하면 Ubuntu 24.04, `/usr/bin/bwrap`, AppArmor namespace 제한, 기존 프로필 부재를 다시 확인한다.
  고정 Ubuntu deb(39,592 bytes)를 HTTPS로 받아 SHA256 검증 후 프로필 하나만 추출한다. 패키지 설치·전역 sysctl 변경·전체 서비스 reload는 하지 않는다.
- 실제 변경은 `/etc/apparmor.d/bwrap-userns-restrict` 신규 생성 및 `bwrap`, `unpriv_bwrap` 두 profile 로드다.
  **모든 `/usr/bin/bwrap` 사용에 영향을 주며 Codex 하나에만 한정되지 않는다.** 기존 정책/충돌은 덮지 않고 중단한다.
- 일반 프로젝트 설정 개정은 sudo가 아니다. 이 설치는 호스트 기동 문제만 다루며, 이후 공식 오버레이와 실제 업무 카나리가 별도로 남는다.

## 실행 — 아래 블록 전체를 서브 일반 터미널에 붙여 넣기

문서나 새 스크립트가 서브에 이미 배달됐다고 가정하지 않는 자체 포함 블록이다.

```bash
(
set -euo pipefail
umask 077
if (( EUID == 0 )); then
  printf '%s\n' 'STOP: run as the normal subnode user; only the profile installation uses sudo.' >&2
  exit 1
fi
cd {{WORKSPACE_SHELL}}
sub_harness_work_dir="$PWD"
/usr/bin/python3 -I -B - "$sub_harness_work_dir" <<'SCOPE_PY'
import json, sys, tomllib
from pathlib import Path
ws = Path(sys.argv[1]).resolve()
cfg = tomllib.loads((ws / '.codex/config.toml').read_text())
if cfg.get('default_permissions') != 'vllm-sub' or 'vllm-sub' not in cfg.get('permissions', {}):
    raise SystemExit('STOP: project is not configured as vllm-sub; use the confirmed subnode terminal')
manifest = ws / 'output/{{TOPOLOGY}}/manifest.yaml'
if not manifest.is_file():
    raise SystemExit('STOP: provisioned sub manifest is absent; return to terraforming')
roles = [line.split(':', 1)[1].split('#', 1)[0].strip().strip(chr(34)).strip(chr(39))
         for line in manifest.read_text().splitlines() if line.startswith('self_role:')]
if roles != ['sub']:
    raise SystemExit('STOP: manifest does not identify this checkout as sub')
print('project_profile=vllm-sub')
SCOPE_PY
mkdir -p "$sub_harness_work_dir/docs/logs"
sub_harness_logdir=$(mktemp -d "$sub_harness_work_dir/docs/logs/harness-environment.XXXXXX")
trap 'sub_harness_rc=$?; printf "operator_exit=%s\nrequest_result_dir=%s\n" "$sub_harness_rc" "$sub_harness_logdir"' EXIT
codex --version | tee "$sub_harness_logdir/codex-version.txt"
set +e
timeout --kill-after=5s 30s codex sandbox --cd "$sub_harness_work_dir" \
  --permission-profile vllm-sub --include-managed-config -- /usr/bin/true \
  >"$sub_harness_logdir/sandbox-before.log" 2>&1
sub_harness_before=$?
set -e
printf 'sandbox_before_exit=%s\n' "$sub_harness_before" | tee "$sub_harness_logdir/before-status.txt"
cat "$sub_harness_logdir/sandbox-before.log"
if [ "$sub_harness_before" -ne 0 ]; then
  . /etc/os-release
  test "$ID:$VERSION_ID" = 'ubuntu:24.04'
  test "$(command -v bwrap)" = /usr/bin/bwrap
  test -x /usr/sbin/apparmor_parser
  test "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns)" = '1'
  /usr/bin/python3 -I -B - "$sub_harness_logdir/sandbox-before.log" <<'FAILURE_PY'
import sys
from pathlib import Path
s = Path(sys.argv[1]).read_text(errors='replace')
if 'bwrap: setting up uid map: Permission denied' not in s:
    raise SystemExit('STOP: failure differs from the reviewed uid-map denial; return the log without applying')
FAILURE_PY
  bwrap --version | tee "$sub_harness_logdir/bwrap-version.txt"
  mkdir "$sub_harness_logdir/package"
  curl --fail --location --silent --show-error --proto '=https' --proto-redir '=https' \
    --connect-timeout 15 --max-time 120 \
    -o "$sub_harness_logdir/package/apparmor-profiles.deb" \
    'https://ports.ubuntu.com/ubuntu-ports/pool/main/a/apparmor/apparmor-profiles_4.0.1really4.0.1-0ubuntu0.24.04.9_all.deb'
  (cd "$sub_harness_logdir/package"
   printf '%s\n' '90b02aa006eea7702cd4e851343e469e41365dda42145a3cb035de1d6c773b8c  apparmor-profiles.deb' | sha256sum --check -
   dpkg-deb --extract apparmor-profiles.deb unpack)
sudo /usr/bin/python3 -I -B - "$sub_harness_logdir/package/unpack/usr/share/apparmor/extra-profiles/bwrap-userns-restrict" <<'PY'
import hashlib, os, stat, subprocess, sys
from pathlib import Path

def require(ok, reason):
    if not ok:
        raise SystemExit("STOP: " + reason)

require(os.geteuid() == 0 and sys.flags.isolated, "root and isolated Python required")
source_fd = os.open(sys.argv[1], os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
with os.fdopen(source_fd, 'rb') as source:
    source_info = os.fstat(source.fileno())
    require(stat.S_ISREG(source_info.st_mode) and source_info.st_size == 1936, 'unexpected candidate file')
    data = source.read(1937)
require(len(data) == 1936 and hashlib.sha256(data).hexdigest() ==
        "11d39094f044f0cda0febb3ad517b830301da6b2ce929664af09ee9e4dd264f9", "profile hash differs")
root = Path('/etc/apparmor.d')
target = root / 'bwrap-userns-restrict'
parser = '/usr/sbin/apparmor_parser'
loaded = Path('/sys/kernel/security/apparmor/profiles')
for directory in (Path('/etc'), root):
    info = directory.lstat()
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == 0 and not info.st_mode & 0o022,
            'unexpected host directory: ' + str(directory))
for name in ('bwrap-userns-restrict', 'local/bwrap-userns-restrict', 'local/unpriv_bwrap',
             'disable/bwrap-userns-restrict', 'force-complain/bwrap-userns-restrict'):
    require(not os.path.lexists(root / name), 'existing policy path: ' + name)
require(not any('bwrap' in line.lower() for line in loaded.read_text().splitlines()),
        'existing loaded bwrap policy; review before adding')
conflicts = []
def walk_error(error):
    raise error
for directory, dirs, files in os.walk(root, onerror=walk_error):
    for name in files:
        path = Path(directory) / name
        if path.is_file() and b'bwrap' in path.read_bytes().lower():
            conflicts.append(str(path))
require(not conflicts, 'existing bwrap policy reference: ' + ', '.join(conflicts))
require(Path('/proc/sys/kernel/apparmor_restrict_unprivileged_userns').read_text().strip() == '1',
        'host userns policy changed; review current state')
subprocess.run([parser, '--skip-kernel-load', '--skip-cache'], input=data, check=True, timeout=30)
fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o644)
with os.fdopen(fd, 'wb') as stream:
    os.fchown(stream.fileno(), 0, 0)
    os.fchmod(stream.fileno(), 0o644)
    stream.write(data)
    stream.flush()
    os.fsync(stream.fileno())
print('profile_file_created=' + str(target), flush=True)
subprocess.run([parser, '--add', '--skip-cache', str(target)], check=True, timeout=30)
lines = set(loaded.read_text().splitlines())
require({'bwrap (enforce)', 'unpriv_bwrap (enforce)'} <= lines, 'expected enforced profiles not confirmed')
print('profile_apply=OK')
PY

else
  printf '%s\n' 'profile_apply=SKIPPED_ALREADY_WORKING'
fi
set +e
timeout --kill-after=5s 30s codex sandbox --cd "$sub_harness_work_dir" \
  --permission-profile vllm-sub --include-managed-config -- /usr/bin/true \
  >"$sub_harness_logdir/sandbox-after.log" 2>&1
sub_harness_after=$?
set -e
cat "$sub_harness_logdir/sandbox-after.log"
printf 'sandbox_after_exit=%s\n' "$sub_harness_after" | tee "$sub_harness_logdir/after-status.txt"
# Observe only the already-declared hint paths; no token or full env/config dump.
/usr/bin/python3 -I -B - "$sub_harness_work_dir" <<'PATHS_PY' | tee "$sub_harness_logdir/hint-scope-observation.json"
import datetime, importlib.util, json, os, pwd, socket, subprocess, sys
from pathlib import Path
from urllib.parse import urlsplit, unquote
ws = Path(sys.argv[1]).resolve()
card = json.loads((ws / 'Agent_Card.json').read_text())
interfaces = [urlsplit(x.get('url', '')) for x in card.get('supportedInterfaces', [])]
endpoints = [x for x in interfaces if x.scheme == 'ssh' and x.hostname and x.username]
if len(endpoints) != 1:
    raise SystemExit('STOP: no unambiguous provisioned SSH endpoint in Card')
ep = endpoints[0]
target = {'host': ep.hostname, 'ssh_user': unquote(ep.username), 'work_dir': str(ws)}
helper = ws / '.claude/skills/terraforming_node/scripts/check_sub_hint_equip.py'
out = {'schema_version': 1, 'topology': '{{TOPOLOGY}}', 'target': target,
       'observed_home_dir': str(Path.home().resolve()),
       'observation_source': 'operator-terminal:terraforming_node/harness-environment-request',
       'observed_utc': datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00', 'Z'),
       'target_source': 'provisioned-Agent_Card.json; main must compare with manifest',
       'observed_hostname': socket.gethostname(), 'observed_user': pwd.getpwuid(os.getuid()).pw_name,
       'status': 'unobserved'}
if '{{TOPOLOGY}}' != 'single':
    out['status'] = 'not_applicable'
elif not helper.is_file():
    out['reason'] = 'hint equipment helper absent; request official overlay'
else:
    spec = importlib.util.spec_from_file_location('hint_equipment', helper)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    raw = m.read_env_key(ws, m.CLONE_DIR_KEY)
    if not raw:
        out['status'] = 'undeclared'
    elif not Path(raw).is_absolute():
        out['status'] = 'invalid_relative_path'
    else:
        clone = Path(raw).resolve(strict=True)
        if clone == ws or ws in clone.parents or clone in ws.parents:
            raise SystemExit('STOP: hint clone overlaps workspace')
        def git(*args):
            r = subprocess.run(['git', '-C', str(clone), *args], capture_output=True, text=True,
                               timeout=15, env={**{k: v for k, v in os.environ.items() if not k.startswith('GIT_')},
                                                'GIT_TERMINAL_PROMPT': '0'})
            if r.returncode:
                raise SystemExit('STOP: hint clone metadata is not readable; no credentials printed')
            return str(Path(r.stdout.strip()).resolve(strict=True))
        if git('rev-parse', '--show-toplevel') != str(clone):
            raise SystemExit('STOP: hint path is not its git root')
        out.update(status='observed', hint_clone_dir=str(clone),
                   git_dir=git('rev-parse', '--absolute-git-dir'),
                   git_common_dir=git('rev-parse', '--path-format=absolute', '--git-common-dir'))
print(json.dumps(out, ensure_ascii=False, indent=2))
PATHS_PY
exit "$sub_harness_after"
)
```

## 성공 판정과 실패 분기

- 기동 성공: `profile_apply=OK` 또는 `SKIPPED_ALREADY_WORKING`, `sandbox_after_exit=0`.
  이것은 **호스트 기동 확인**이다. 실제 Codex A2A·업무 권한·hint 발행 성공을 뜻하지 않는다.
- `hint-scope-observation.json`의 `status=observed`는 경로 관측이다. 메인이 기존 유효 승인에 연결해
  exact scope를 렌더하고 공식 배달한다. 원 관측 JSON은 보존하고, renderer 입력에는 아래 reference가 지정한 관측 키만 선택해 승인 3키를 연결한다. `undeclared`/`unobserved`는 새 승인 없이 임의 경로를 만들라는 뜻이 아니다.
- 기존 profile/override/다른 bwrap 정책, 다른 OS/실패 문구, package/hash 오류면 중단한다.
  충돌 검사는 알려진 경로·loaded 이름·파일 내 bwrap 참조를 보수적으로 검사하며 모든 wildcard/변수 attachment 해석을 증명하지 않는다.
- 파일 생성 뒤 kernel load/카나리가 실패하면 **부분 적용**이다. 자동 삭제·재설치하지 말고 출력과 결과 디렉터리를 회수한다.
  회수/롤백은 실제 생성 파일·해시·로드 상태·다른 bwrap 사용자 영향을 확인한 별도 request로 수행한다.
- named profile이 불명확하거나 프로젝트 신뢰/상위 managed 거부면 설정을 우회하지 말고 그대로 회신한다.

## 회수물과 다음 작업

터미널 출력(`operator_exit`, `request_result_dir` 포함), `sandbox-before.log`, `sandbox-after.log`,
`before-status.txt`, `after-status.txt`, `hint-scope-observation.json`을 메인에 전달한다.
토큰·인증 파일·전체 사용자 설정·sudo 비밀번호는 전달하지 않는다. 중간 중단이면 생성된 파일만 회수한다.
테라포밍은 회수 → exact 경로 승인 연결 → 공식 설정/런타임 배달 → 요청한 provider/model의 discovery·permissions
카나리 → 캠페인 재개 순으로 진행한다. 같은 sudo 설치를 캠페인마다 반복하지 않는다.

예상 소요: 3~10분, 작은 공식 패키지 다운로드 외 모델 다운로드·GPU 사용 없음. 이 문서의 구문·핀·비로드 검증과
실제 sudo 적용 검증은 구분한다. 실제 호스트 적용은 아직 수행하지 않았으며 운영자 회수로 확인한다.

근거: [Codex Linux prerequisites](https://learn.chatgpt.com/docs/sandboxing#prerequisites),
[Ubuntu user namespace restrictions](https://documentation.ubuntu.com/release-notes/24.04/#unprivileged-user-namespace-restrictions).
