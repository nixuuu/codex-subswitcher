"""Create ONLY synthetic identities in an explicitly supplied scratch directory."""
import base64, hashlib, json, os, pathlib, sys, time
root = pathlib.Path(sys.argv[1]).resolve()
root.mkdir(parents=True, exist_ok=True)
os.chmod(root, 0o700)
(root / 'accounts').mkdir(exist_ok=True)
os.chmod(root / 'accounts', 0o700)
def jwt(value):
    return 'e30.' + base64.urlsafe_b64encode(json.dumps(value).encode()).decode().rstrip('=') + '.demo'
for user, plan in [
    ('alex.work', 'plus'),
    ('alex.personal', 'pro'),
    ('backend.team', 'business'),
    ('long.project.account.name', 'plus'),
    ('frontend.team', 'pro'),
    ('backup.account', 'pro'),
]:
    workspace = 'demo-' + user
    ident = hashlib.sha256((workspace + '\0' + user).encode()).hexdigest()
    value = {'auth_mode':'chatgpt','OPENAI_API_KEY':None,'tokens':{
        'id_token':jwt({'sub':user,'email':user+'@example.test','https://api.openai.com/auth':{'chatgpt_account_id':workspace,'chatgpt_plan_type':plan}}),
        'access_token':jwt({'exp':int(time.time())+3600}), 'refresh_token':'synthetic-not-a-token', 'account_id':workspace}}
    path = root / 'accounts' / (ident + '.json')
    path.write_text(json.dumps(value))
    os.chmod(path,0o600)
(root/'active').write_text(ident)
os.chmod(root/'active',0o600)
