#!/usr/bin/python3
"""Explicit source-version compatibility observer. Never a native/default fallback.

Business stdin must be DEVNULL. Caller captures real stdout/stderr; only the
exclusive --result sidecar contains typed observations. No Root temp imports.
"""
from __future__ import annotations
import argparse
import datetime
import hashlib
import importlib.util
import json
import math
import os
import pathlib
import stat
import sys

sys.dont_write_bytecode = True
MODE = 'SourceVersionCompat'
SCHEMA = 'topology-legacy-api/1'
OPERATIONS = frozenset(('read_layer_header','layer_files','evaluate','active_campaign_id','read_declaration','campaigns_dir','derive_path'))
GLOBAL_NAMES = ('EXIT_OK','EXIT_USAGE','EXIT_CONTRACT_VIOLATION','LAYER_SUFFIX','LAYER_PATHSPEC','LAYER_EXCLUDE_PATHSPEC','HEADER_WINDOW','BRANCH_TO_TOPOLOGY','TOPOLOGIES','REPO_ROOT','CAMPAIGNS','TEMPLATE','ACTIVE_POINTER','BOOTSTRAP','RESERVED_IDS','PURGE_KEEPS')
PUBLIC_ERROR_FIELDS = ('name','obj','errno','strerror','filename','filename2','winerror','characters_written','code','message','field','url','reason','line','partial','expected','encoding','object','start','end','msg','doc','pattern','pos','lineno','colno','offset','end_lineno','end_offset','text','print_file_and_line','verify_code','verify_message','library','context','context_mark','problem','problem_mark','note','position','character')

class TransportError(Exception): pass
class ObservationError(Exception): pass

def require(ok, message):
    if not ok:
        raise TransportError(message)

def unique_pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, 'duplicate JSON key: '+key)
        result[key] = value
    return result

def invalid_constant(text):
    raise TransportError('nonfinite untagged JSON constant: '+text)

def integer_text(number):
    """Generic arbitrary-size DTO encoding without changing interpreter digit limits."""
    if number == 0:
        return '0'
    negative = number < 0
    number = -number if negative else number
    chunks = []
    while number:
        number, remainder = divmod(number, 1000000000)
        chunks.append(remainder)
    return ('-' if negative else '')+str(chunks[-1])+''.join(format(x,'09d') for x in reversed(chunks[:-1]))

def integer_value(text):
    require(type(text) is str and bool(text), 'int DTO requires decimal text')
    negative = text.startswith('-')
    digits = text[1:] if negative else text
    require(bool(digits) and all('0' <= c <= '9' for c in digits), 'int DTO requires ASCII digits')
    value = 0
    for start in range(0,len(digits),9):
        part = digits[start:start+9]
        value = value*(10**len(part))+int(part)
    return -value if negative else value

def decode(dto):
    require(type(dto) is list and bool(dto) and type(dto[0]) is str, 'tagged input DTO required')
    tag = dto[0]
    if tag == 'none':
        require(len(dto)==1, 'none DTO arity')
        return None
    require(len(dto) in (2,3), 'input DTO arity')
    data = dto[1]
    if tag == 'set':
        require(type(data) is list, 'set members required')
        if len(dto)==3:
            order = dto[2]
            if order == 'sorted':
                order = list(range(len(data)))
            require(type(order) is list and all(type(x) is int for x in order) and sorted(order)==list(range(len(data))), 'set input order must be a permutation')
        else:
            order = list(range(len(data)))
        return set(decode(data[i]) for i in order)
    require(len(dto)==2, 'non-set input DTO arity')
    if tag == 'bool':
        require(type(data) is bool, 'bool DTO is not int')
        return data
    if tag == 'str':
        require(type(data) is str, 'str DTO payload')
        return data
    if tag == 'int':
        return integer_value(data)
    if tag == 'float':
        require(type(data) is str, 'float repr text required')
        return float(data)
    if tag == 'bytes':
        require(type(data) is str and len(data)%2==0 and all(c in '0123456789abcdefABCDEF' for c in data), 'bytes hex required')
        return bytes.fromhex(data)
    if tag in ('date','datetime'):
        require(type(data) is str, 'ISO date/datetime text required')
        return (datetime.date if tag=='date' else datetime.datetime).fromisoformat(data)
    if tag in ('list','tuple'):
        require(type(data) is list, 'sequence DTO list required')
        values = [decode(x) for x in data]
        return values if tag=='list' else tuple(values)
    if tag == 'dict':
        require(type(data) is list, 'dict ordered pairs required')
        result = {}
        for pair in data:
            require(type(pair) is list and len(pair)==2, 'dict DTO pair required')
            key = decode(pair[0])
            require(key not in result, 'input dict has duplicate Python key')
            result[key] = decode(pair[1])
        return result
    if tag == 'path':
        if type(data) is str:
            return pathlib.Path(data)
        require(type(data) is dict and set(data)=={'module','type','value'}, 'Path public shape required')
        require(data['module']=='pathlib' and type(data['value']) is str, 'Path module/value required')
        constructors = {name:getattr(pathlib,name) for name in ('Path','PosixPath','WindowsPath','PurePath','PurePosixPath','PureWindowsPath')}
        require(data['type'] in constructors, 'unknown pathlib class')
        return constructors[data['type']](data['value'])
    raise TransportError('unsupported input DTO tag: '+tag)

def encode(value, active=None):
    active = set() if active is None else active
    if value is None: return ['none']
    if type(value) is bool: return ['bool',value]
    if type(value) is int: return ['int',integer_text(value)]
    if type(value) is float: return ['float',repr(value)]
    if type(value) is str: return ['str',value]
    if type(value) is bytes: return ['bytes',value.hex()]
    if type(value) is datetime.datetime: return ['datetime',value.isoformat()]
    if type(value) is datetime.date: return ['date',value.isoformat()]
    if isinstance(value,pathlib.PurePath):
        return ['path',dict(module=type(value).__module__,type=type(value).__name__,value=str(value))]
    if id(value) in active:
        raise ObservationError('cyclic observed object: '+type(value).__name__)
    active.add(id(value))
    try:
        if isinstance(value,BaseException):
            return ['exception',exception(value,active,already_active=True)]
        if type(value).__module__=='yaml.error' and type(value).__name__=='Mark':
            fields = ('name','index','line','column','buffer','pointer')
            public = {k:v for k,v in vars(value).items() if not k.startswith('_')}
            if not all(k in public for k in fields):
                raise ObservationError('incomplete yaml.error.Mark public shape')
            return ['yaml_mark',dict(module=type(value).__module__,type=type(value).__name__,fields={k:encode(public[k],active) for k in fields},extra_attributes={k:encode(v,active) for k,v in public.items() if k not in fields})]
        if type(value) is dict:
            return ['dict',[[encode(k,active),encode(v,active)] for k,v in value.items()]]
        if type(value) in (list,tuple,set):
            items = [encode(x,active) for x in value]
            if type(value) is set:
                items.sort(key=lambda x:json.dumps(x,ensure_ascii=True,separators=(',',':')))
            return [type(value).__name__,items]
        raise ObservationError('unsupported observed type: '+type(value).__module__+'.'+type(value).__name__)
    finally:
        active.remove(id(value))

def exception(error, active=None, already_active=False):
    active = set() if active is None else active
    if not already_active:
        if id(error) in active: raise ObservationError('cyclic exception graph')
        active.add(id(error))
    try:
        value = lambda x:encode(x,active)
        cause = error.__cause__
        context = error.__context__
        relation = 'cause' if cause is not None else 'context' if context is not None and not error.__suppress_context__ else None
        row = dict(type=type(error).__name__,module=type(error).__module__,message=str(error),args=[value(x) for x in error.args],suppress_context=error.__suppress_context__,relation=relation,attributes={},frames=[])
        for name in PUBLIC_ERROR_FIELDS:
            try: item = getattr(error,name)
            except AttributeError: continue
            row['attributes'][name] = value(item)
        for name,item in vars(error).items():
            if not name.startswith('_') and name not in row['attributes']:
                row['attributes'][name] = value(item)
        if hasattr(error,'__notes__'):
            row['notes'] = value(error.__notes__)
        ctor = dict(kind=type(error).__name__,module=type(error).__module__,args=[value(x) for x in error.args])
        fields = None
        if isinstance(error,(UnicodeEncodeError,UnicodeDecodeError,UnicodeTranslateError)):
            fields = ['encoding','object','start','end','reason'] if hasattr(error,'encoding') else ['object','start','end','reason']
        elif type(error).__module__=='json.decoder' and type(error).__name__=='JSONDecodeError':
            fields = ['msg','doc','pos']
        elif type(error).__module__=='re' and type(error).__name__=='error':
            fields = ['msg','pattern','pos']
        elif any(t.__module__=='yaml.error' and t.__name__=='MarkedYAMLError' for t in type(error).__mro__):
            fields = ['context','context_mark','problem','problem_mark','note']
        elif type(error).__module__=='yaml.reader' and type(error).__name__=='ReaderError':
            fields = ['name','position','character','encoding','reason']
        elif type(error).__name__=='ContractViolation' and all(hasattr(error,n) for n in ('code','message','field')):
            fields = ['code','message','field']
        elif isinstance(error,OSError):
            fields = [n for n in ('errno','strerror','filename','filename2','winerror') if hasattr(error,n)]
        if fields is not None: ctor['fields'] = {n:value(getattr(error,n)) for n in fields}
        if isinstance(error,SystemExit): ctor['code'] = value(error.code)
        row['constructor'] = ctor
        tb = error.__traceback__
        while tb is not None:
            row['frames'].append(dict(file=tb.tb_frame.f_code.co_filename,line=tb.tb_lineno,function=tb.tb_frame.f_code.co_name))
            tb = tb.tb_next
        # Actual cause/context are both retained. The visible chain remains explicit,
        # including the suppression flag; no logical/old observer frames are spliced.
        row['cause'] = exception(cause,active) if cause is not None else None
        row['context'] = exception(context,active) if context is not None else None
        if relation is not None: row['next'] = row[relation]
        return row
    finally:
        if not already_active: active.remove(id(error))

def nodes(args,kwargs):
    result = []
    active = set()
    def visit(value,path):
        result.append((value,path))
        if type(value) not in (list,tuple,dict,set): return
        if id(value) in active: raise ObservationError('cyclic input graph')
        active.add(id(value))
        try:
            if type(value) is dict:
                for i,(key,item) in enumerate(value.items()):
                    visit(key,path+[dict(kind='dict_key',index=i)])
                    visit(item,path+[dict(kind='dict_value',index=i)])
            else:
                for i,item in enumerate(value): visit(item,path+[dict(kind=type(value).__name__+'_item',index=i)])
        finally: active.remove(id(value))
    visit(args,[dict(kind='args')]); visit(kwargs,[dict(kind='kwargs')])
    return result

def globals_of(module):
    return {name:encode(vars(module)[name]) for name in GLOBAL_NAMES if name in vars(module)}

def source_pin(path):
    p = pathlib.Path(path)
    data = p.read_bytes()
    return dict(path=str(p),sha256=hashlib.sha256(data).hexdigest(),mode=stat.S_IMODE(p.stat().st_mode),size=len(data))

def error_info(error):
    return dict(type=type(error).__name__,module=type(error).__module__,message=str(error))

def json_value(value):
    if value is None or type(value) in (str,bool,int): return
    if type(value) is float:
        require(math.isfinite(value), 'nonfinite untagged JSON number')
        return
    if type(value) is list:
        for item in value: json_value(item)
        return
    require(type(value) is dict and all(type(k) is str for k in value), 'JSON tree required')
    for item in value.values(): json_value(item)

def stdin_metadata():
    actual = os.fstat(0)
    null = os.stat('/dev/null')
    return dict(kind='DEVNULL' if stat.S_ISCHR(actual.st_mode) and (actual.st_dev,actual.st_ino)==(null.st_dev,null.st_ino) else 'OTHER',mode=stat.S_IMODE(actual.st_mode),device=actual.st_dev,inode=actual.st_ino,rdev=actual.st_rdev)

def validate_request(request):
    require(type(request) is dict, 'request must be JSON object')
    json_value(request)
    keys = {'schema','mode','id','module_path','module_alias','operation','args_dto','kwargs_dto'}
    require(keys <= set(request) and set(request) <= keys|{'logical_source_context'}, 'exact required request fields plus optional logical_source_context')
    require(request['schema']==SCHEMA and request['mode']==MODE, 'explicit SourceVersionCompat schema/mode required')
    require(type(request['module_path']) is str and os.path.isabs(request['module_path']) and '\x00' not in request['module_path'], 'absolute real compatibility module path required')
    alias = request['module_alias']
    require(type(alias) is str and alias.isidentifier() and alias!='__main__' and alias not in sys.modules, 'fresh declared module alias required')
    require(type(request['operation']) is str and request['operation'] in OPERATIONS, 'operation outside seven readonly callable names')
    return request

def observe(request):
    record = dict(schema=SCHEMA,mode=MODE,origin='LegacySourceVariant',native_execution=False,baseline_replay=False,id=request['id'],observer_pid=os.getpid(),module_path=request['module_path'],module_alias=request['module_alias'],operation=request['operation'],phase='decode_request',module_load_attempted=False,call_attempted=False,observation_complete=False,collector_errors=[],logical_source_context=request.get('logical_source_context'),frame_provenance='actual current Python frames; not original-baseline or Rust OS frames')
    record['stdin_observation'] = stdin_metadata()
    module = None
    args = kwargs = None
    business_error = import_error = None
    try:
        args = decode(request['args_dto']); kwargs = decode(request['kwargs_dto'])
        require(type(args) in (list,tuple) and type(kwargs) is dict and all(type(k) is str for k in kwargs), 'args sequence/kwargs string-key dict required')
        record['input_before'] = dict(args=encode(args),kwargs=encode(kwargs))
        graph = nodes(args,kwargs)
        record['input_nodes'] = [dict(path=path,type=type(obj).__name__,value=encode(obj)) for obj,path in graph]
        record['phase'] = 'source_before_observation'
        record['module_source_before'] = source_pin(request['module_path'])
        record['sys_path_before'] = list(sys.path)
        record['phase'] = 'module_import'
        record['module_load_attempted'] = True
        try:
            spec = importlib.util.spec_from_file_location(request['module_alias'],request['module_path'])
            require(spec is not None and spec.loader is not None, 'module loader unavailable')
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
        except BaseException as error:
            import_error = error
        if module is not None:
            record['module_file'] = module.__file__
            record['module_cached_path'] = getattr(module,'__cached__',None)
        if import_error is not None:
            record['observed_outcome'] = 'module_import_raised'
            record.update(kind='import_exception',exception=exception(import_error))
        else:
            record['module_file'] = module.__file__
            record['module_cached_path'] = getattr(module,'__cached__',None)
            record['globals_before'] = globals_of(module)
            record['phase'] = 'function_call'
            record['call_attempted'] = True
            try:
                value = getattr(module,request['operation'])(*args,**kwargs)
            except BaseException as error:
                business_error = error
            if business_error is not None:
                record['observed_outcome'] = 'function_raised'
                record.update(kind='function_exception',exception=exception(business_error))
            else:
                record['observed_outcome'] = 'function_returned'
                record.update(kind='value',value=encode(value),return_aliases=[path for obj,path in graph if obj is value])
        record['phase'] = 'after_observation'
        record['args_after'] = encode(args)
        record['kwargs_after'] = encode(kwargs)
        record['globals_after'] = globals_of(module) if module is not None else None
        record['module_source_after'] = source_pin(request['module_path'])
        record['source_endpoints_equal'] = record['module_source_before']==record['module_source_after']
        record['sys_path_after'] = list(sys.path)
        record['observation_complete'] = True
    except BaseException as error:
        # Never serialize an observer failure as a business exception or fabricate attrs.
        record['collector_errors'].append(dict(phase=record['phase'],error=error_info(error)))
        record['kind'] = 'collector_incomplete'
        record['observation_complete'] = False
        if business_error is not None: record['unserialized_function_exception'] = dict(type=type(business_error).__name__,module=type(business_error).__module__)
        if import_error is not None: record['unserialized_import_exception'] = dict(type=type(import_error).__name__,module=type(import_error).__module__)
    return record

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mode',required=True,choices=[MODE])
    parser.add_argument('--request',required=True)
    parser.add_argument('--result',required=True)
    opts = parser.parse_args(argv)
    # Reserve the result before any target import. Existing result means no dispatch.
    try:
        require(os.path.isabs(opts.request) and os.path.isabs(opts.result), 'absolute request/result paths required')
        descriptor = os.open(opts.result,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    except BaseException:
        return 78
    status = 77
    try:
        with open(opts.request,'r',encoding='utf-8') as stream:
            request = validate_request(json.load(stream,object_pairs_hook=unique_pairs,parse_constant=invalid_constant))
        require(stdin_metadata()['kind']=='DEVNULL', 'business stdin must be actual DEVNULL')
        record = observe(request)
        status = 0 if record['observation_complete'] else 77
    except BaseException as error:
        record = dict(schema=SCHEMA,mode=MODE,origin='LegacySourceVariant',native_execution=False,baseline_replay=False,observer_pid=os.getpid(),phase='request_transport',module_load_attempted=False,call_attempted=False,observation_complete=False,kind='transport_error',collector_errors=[error_info(error)])
    try:
        data = (json.dumps(record,ensure_ascii=True,allow_nan=False,indent=2)+'\n').encode('ascii')
        with os.fdopen(descriptor,'wb') as stream:
            stream.write(data); stream.flush(); os.fsync(stream.fileno())
    except BaseException:
        try: os.close(descriptor)
        except OSError: pass
        return 78
    return status

if __name__=='__main__':
    raise SystemExit(main())
