"""JSON transport for run_bench's dataset arguments; computation belongs to Rust."""
import ast
import builtins
import json
import os
from pathlib import Path
import subprocess
import sys

_LAUNCHER=Path(os.environ.get('DATASET_PREP_API_LAUNCHER') or Path(__file__).resolve().with_name('dataset_prep_api.sh'))


def _raise(record):
    name,message=record['type'],record['message']
    if record.get('stderr'):sys.stderr.write(record['stderr'])
    if name=='SystemExit':raise SystemExit(int(message) if message.lstrip('-').isdigit() else message)
    base=getattr(builtins,name,None)
    if not isinstance(base,type) or not issubclass(base,Exception):raise RuntimeError('dataset native error %s: %s' % (name,message))
    if base is KeyError:
        try:key=ast.literal_eval(message)
        except (ValueError,SyntaxError):key=message
        raise KeyError(key)
    try:error=base(message)
    except TypeError:
        reported=type(name,(base,),{'__str__':lambda self:self.reported_message,'__module__':base.__module__})
        error=reported.__new__(reported);BaseException.__init__(error,message);error.reported_message=message
    raise error


def _call(operation,arguments):
    row=json.dumps(dict(id='call',operation=operation,arguments=arguments),ensure_ascii=True)+'\n'
    cp=subprocess.run(['bash',str(_LAUNCHER),'evaluate-jsonl'],input=row,capture_output=True,text=True)
    if cp.returncode or cp.stderr:raise RuntimeError('dataset native transport failed (%s): %s' % (cp.returncode,cp.stderr.strip()))
    records=[json.loads(line) for line in cp.stdout.splitlines() if line.strip()]
    if len(records)!=1 or not isinstance(records[0],dict):raise RuntimeError('dataset native transport expected one object row')
    record=records[0]
    if record.get('id')!='call' or record.get('kind') not in ('value','exception'):raise RuntimeError('dataset native transport returned the wrong row')
    if record['kind']=='exception':_raise(record)
    if 'value' not in record:raise RuntimeError('dataset native transport returned no value')
    if record.get('stderr'):sys.stderr.write(record['stderr'])
    return record['value']


def data_arg(kind,path):
    return _call('data_arg',dict(kind=kind,path=path))


def column_mapper_arg(spec,with_counts):
    return _call('column_mapper_arg',dict(spec=spec,with_counts=with_counts))


def target_rows(rows,input_len):
    return tuple(_call('target_rows',dict(rows=rows,input_len=input_len)))
