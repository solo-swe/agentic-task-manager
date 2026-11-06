# Generated from atm-cli/atm/ by atm-cli/build.py. Don't edit this file: edit the modules and run
# `pnpm run build:cli` in dashboard/ (the pre-commit hook does it too).
"""atm: the Agentic Task Manager helper that the skills run.

Run through the `atm` launcher, which checks for Python 3.9+. Standard library only: the plugin is used as cloned,
with nothing to install.
"""

from __future__ import absolute_import, annotations
import base64
import contextlib
import copy
import errno
import hashlib
import json
import os
import random
import re
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
from datetime import datetime, timedelta, timezone
from fnmatch import fnmatchcase
from typing import Annotated, Any, Callable, Dict, List, Literal, Optional, Protocol, Tuple, TypedDict
from urllib.parse import parse_qs, urlsplit

sys.path.insert(0, os.path.join(os.path.dirname(os.path.realpath(__file__)), "vendor"))
import click
import typer


# ==== vendored six ====
# Utilities for writing code that runs on Python 2 and 3

# Copyright (c) 2010-2024 Benjamin Peterson
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.



import functools
import itertools
import operator
import sys
import types

__author__ = "Benjamin Peterson <benjamin@python.org>"
__version__ = "1.17.0"


# Useful for very coarse version differentiation.
PY2 = sys.version_info[0] == 2
PY3 = sys.version_info[0] == 3
PY34 = sys.version_info[0:2] >= (3, 4)

if PY3:
    string_types = str,
    integer_types = int,
    class_types = type,
    text_type = str
    binary_type = bytes

    MAXSIZE = sys.maxsize
else:
    string_types = basestring,
    integer_types = (int, long)
    class_types = (type, types.ClassType)
    text_type = unicode
    binary_type = str

    if sys.platform.startswith("java"):
        # Jython always uses 32 bits.
        MAXSIZE = int((1 << 31) - 1)
    else:
        # It's possible to have sizeof(long) != sizeof(Py_ssize_t).
        class X(object):

            def __len__(self):
                return 1 << 31
        try:
            len(X())
        except OverflowError:
            # 32-bit
            MAXSIZE = int((1 << 31) - 1)
        else:
            # 64-bit
            MAXSIZE = int((1 << 63) - 1)
        del X

if PY34:
    from importlib.util import spec_from_loader
else:
    spec_from_loader = None


def _add_doc(func, doc):
    """Add documentation to a function."""
    func.__doc__ = doc


def _import_module(name):
    """Import module, returning the module after the last dot."""
    __import__(name)
    return sys.modules[name]


class _LazyDescr(object):

    def __init__(self, name):
        self.name = name

    def __get__(self, obj, tp):
        result = self._resolve()
        setattr(obj, self.name, result)  # Invokes __set__.
        try:
            # This is a bit ugly, but it avoids running this again by
            # removing this descriptor.
            delattr(obj.__class__, self.name)
        except AttributeError:
            pass
        return result


class MovedModule(_LazyDescr):

    def __init__(self, name, old, new=None):
        super(MovedModule, self).__init__(name)
        if PY3:
            if new is None:
                new = name
            self.mod = new
        else:
            self.mod = old

    def _resolve(self):
        return _import_module(self.mod)

    def __getattr__(self, attr):
        _module = self._resolve()
        value = getattr(_module, attr)
        setattr(self, attr, value)
        return value


class _LazyModule(types.ModuleType):

    def __init__(self, name):
        super(_LazyModule, self).__init__(name)
        self.__doc__ = self.__class__.__doc__

    def __dir__(self):
        attrs = ["__doc__", "__name__"]
        attrs += [attr.name for attr in self._moved_attributes]
        return attrs

    # Subclasses should override this
    _moved_attributes = []


class MovedAttribute(_LazyDescr):

    def __init__(self, name, old_mod, new_mod, old_attr=None, new_attr=None):
        super(MovedAttribute, self).__init__(name)
        if PY3:
            if new_mod is None:
                new_mod = name
            self.mod = new_mod
            if new_attr is None:
                if old_attr is None:
                    new_attr = name
                else:
                    new_attr = old_attr
            self.attr = new_attr
        else:
            self.mod = old_mod
            if old_attr is None:
                old_attr = name
            self.attr = old_attr

    def _resolve(self):
        module = _import_module(self.mod)
        return getattr(module, self.attr)


class _SixMetaPathImporter(object):

    """
    A meta path importer to import six.moves and its submodules.

    This class implements a PEP302 finder and loader. It should be compatible
    with Python 2.5 and all existing versions of Python3
    """

    def __init__(self, six_module_name):
        self.name = six_module_name
        self.known_modules = {}

    def _add_module(self, mod, *fullnames):
        for fullname in fullnames:
            self.known_modules[self.name + "." + fullname] = mod

    def _get_module(self, fullname):
        return self.known_modules[self.name + "." + fullname]

    def find_module(self, fullname, path=None):
        if fullname in self.known_modules:
            return self
        return None

    def find_spec(self, fullname, path, target=None):
        if fullname in self.known_modules:
            return spec_from_loader(fullname, self)
        return None

    def __get_module(self, fullname):
        try:
            return self.known_modules[fullname]
        except KeyError:
            raise ImportError("This loader does not know module " + fullname)

    def load_module(self, fullname):
        try:
            # in case of a reload
            return sys.modules[fullname]
        except KeyError:
            pass
        mod = self.__get_module(fullname)
        if isinstance(mod, MovedModule):
            mod = mod._resolve()
        else:
            mod.__loader__ = self
        sys.modules[fullname] = mod
        return mod

    def is_package(self, fullname):
        """
        Return true, if the named module is a package.

        We need this method to get correct spec objects with
        Python 3.4 (see PEP451)
        """
        return hasattr(self.__get_module(fullname), "__path__")

    def get_code(self, fullname):
        """Return None

        Required, if is_package is implemented"""
        self.__get_module(fullname)  # eventually raises ImportError
        return None
    get_source = get_code  # same as get_code

    def create_module(self, spec):
        return self.load_module(spec.name)

    def exec_module(self, module):
        pass

_importer = _SixMetaPathImporter(__name__)


class _MovedItems(_LazyModule):

    """Lazy loading of moved objects"""
    __path__ = []  # mark as package


_moved_attributes = [
    MovedAttribute("cStringIO", "cStringIO", "io", "StringIO"),
    MovedAttribute("filter", "itertools", "builtins", "ifilter", "filter"),
    MovedAttribute("filterfalse", "itertools", "itertools", "ifilterfalse", "filterfalse"),
    MovedAttribute("input", "__builtin__", "builtins", "raw_input", "input"),
    MovedAttribute("intern", "__builtin__", "sys"),
    MovedAttribute("map", "itertools", "builtins", "imap", "map"),
    MovedAttribute("getcwd", "os", "os", "getcwdu", "getcwd"),
    MovedAttribute("getcwdb", "os", "os", "getcwd", "getcwdb"),
    MovedAttribute("getoutput", "commands", "subprocess"),
    MovedAttribute("range", "__builtin__", "builtins", "xrange", "range"),
    MovedAttribute("reload_module", "__builtin__", "importlib" if PY34 else "imp", "reload"),
    MovedAttribute("reduce", "__builtin__", "functools"),
    MovedAttribute("shlex_quote", "pipes", "shlex", "quote"),
    MovedAttribute("StringIO", "StringIO", "io"),
    MovedAttribute("UserDict", "UserDict", "collections", "IterableUserDict", "UserDict"),
    MovedAttribute("UserList", "UserList", "collections"),
    MovedAttribute("UserString", "UserString", "collections"),
    MovedAttribute("xrange", "__builtin__", "builtins", "xrange", "range"),
    MovedAttribute("zip", "itertools", "builtins", "izip", "zip"),
    MovedAttribute("zip_longest", "itertools", "itertools", "izip_longest", "zip_longest"),
    MovedModule("builtins", "__builtin__"),
    MovedModule("configparser", "ConfigParser"),
    MovedModule("collections_abc", "collections", "collections.abc" if sys.version_info >= (3, 3) else "collections"),
    MovedModule("copyreg", "copy_reg"),
    MovedModule("dbm_gnu", "gdbm", "dbm.gnu"),
    MovedModule("dbm_ndbm", "dbm", "dbm.ndbm"),
    MovedModule("_dummy_thread", "dummy_thread", "_dummy_thread" if sys.version_info < (3, 9) else "_thread"),
    MovedModule("http_cookiejar", "cookielib", "http.cookiejar"),
    MovedModule("http_cookies", "Cookie", "http.cookies"),
    MovedModule("html_entities", "htmlentitydefs", "html.entities"),
    MovedModule("html_parser", "HTMLParser", "html.parser"),
    MovedModule("http_client", "httplib", "http.client"),
    MovedModule("email_mime_base", "email.MIMEBase", "email.mime.base"),
    MovedModule("email_mime_image", "email.MIMEImage", "email.mime.image"),
    MovedModule("email_mime_multipart", "email.MIMEMultipart", "email.mime.multipart"),
    MovedModule("email_mime_nonmultipart", "email.MIMENonMultipart", "email.mime.nonmultipart"),
    MovedModule("email_mime_text", "email.MIMEText", "email.mime.text"),
    MovedModule("BaseHTTPServer", "BaseHTTPServer", "http.server"),
    MovedModule("CGIHTTPServer", "CGIHTTPServer", "http.server"),
    MovedModule("SimpleHTTPServer", "SimpleHTTPServer", "http.server"),
    MovedModule("cPickle", "cPickle", "pickle"),
    MovedModule("queue", "Queue"),
    MovedModule("reprlib", "repr"),
    MovedModule("socketserver", "SocketServer"),
    MovedModule("_thread", "thread", "_thread"),
    MovedModule("tkinter", "Tkinter"),
    MovedModule("tkinter_dialog", "Dialog", "tkinter.dialog"),
    MovedModule("tkinter_filedialog", "FileDialog", "tkinter.filedialog"),
    MovedModule("tkinter_scrolledtext", "ScrolledText", "tkinter.scrolledtext"),
    MovedModule("tkinter_simpledialog", "SimpleDialog", "tkinter.simpledialog"),
    MovedModule("tkinter_tix", "Tix", "tkinter.tix"),
    MovedModule("tkinter_ttk", "ttk", "tkinter.ttk"),
    MovedModule("tkinter_constants", "Tkconstants", "tkinter.constants"),
    MovedModule("tkinter_dnd", "Tkdnd", "tkinter.dnd"),
    MovedModule("tkinter_colorchooser", "tkColorChooser",
                "tkinter.colorchooser"),
    MovedModule("tkinter_commondialog", "tkCommonDialog",
                "tkinter.commondialog"),
    MovedModule("tkinter_tkfiledialog", "tkFileDialog", "tkinter.filedialog"),
    MovedModule("tkinter_font", "tkFont", "tkinter.font"),
    MovedModule("tkinter_messagebox", "tkMessageBox", "tkinter.messagebox"),
    MovedModule("tkinter_tksimpledialog", "tkSimpleDialog",
                "tkinter.simpledialog"),
    MovedModule("urllib_parse", __name__ + ".moves.urllib_parse", "urllib.parse"),
    MovedModule("urllib_error", __name__ + ".moves.urllib_error", "urllib.error"),
    MovedModule("urllib", __name__ + ".moves.urllib", __name__ + ".moves.urllib"),
    MovedModule("urllib_robotparser", "robotparser", "urllib.robotparser"),
    MovedModule("xmlrpc_client", "xmlrpclib", "xmlrpc.client"),
    MovedModule("xmlrpc_server", "SimpleXMLRPCServer", "xmlrpc.server"),
]
# Add windows specific modules.
if sys.platform == "win32":
    _moved_attributes += [
        MovedModule("winreg", "_winreg"),
    ]

for attr in _moved_attributes:
    setattr(_MovedItems, attr.name, attr)
    if isinstance(attr, MovedModule):
        _importer._add_module(attr, "moves." + attr.name)
del attr

_MovedItems._moved_attributes = _moved_attributes

moves = _MovedItems(__name__ + ".moves")
_importer._add_module(moves, "moves")


class Module_six_moves_urllib_parse(_LazyModule):

    """Lazy loading of moved objects in six.moves.urllib_parse"""


_urllib_parse_moved_attributes = [
    MovedAttribute("ParseResult", "urlparse", "urllib.parse"),
    MovedAttribute("SplitResult", "urlparse", "urllib.parse"),
    MovedAttribute("parse_qs", "urlparse", "urllib.parse"),
    MovedAttribute("parse_qsl", "urlparse", "urllib.parse"),
    MovedAttribute("urldefrag", "urlparse", "urllib.parse"),
    MovedAttribute("urljoin", "urlparse", "urllib.parse"),
    MovedAttribute("urlparse", "urlparse", "urllib.parse"),
    MovedAttribute("urlsplit", "urlparse", "urllib.parse"),
    MovedAttribute("urlunparse", "urlparse", "urllib.parse"),
    MovedAttribute("urlunsplit", "urlparse", "urllib.parse"),
    MovedAttribute("quote", "urllib", "urllib.parse"),
    MovedAttribute("quote_plus", "urllib", "urllib.parse"),
    MovedAttribute("unquote", "urllib", "urllib.parse"),
    MovedAttribute("unquote_plus", "urllib", "urllib.parse"),
    MovedAttribute("unquote_to_bytes", "urllib", "urllib.parse", "unquote", "unquote_to_bytes"),
    MovedAttribute("urlencode", "urllib", "urllib.parse"),
    MovedAttribute("splitquery", "urllib", "urllib.parse"),
    MovedAttribute("splittag", "urllib", "urllib.parse"),
    MovedAttribute("splituser", "urllib", "urllib.parse"),
    MovedAttribute("splitvalue", "urllib", "urllib.parse"),
    MovedAttribute("uses_fragment", "urlparse", "urllib.parse"),
    MovedAttribute("uses_netloc", "urlparse", "urllib.parse"),
    MovedAttribute("uses_params", "urlparse", "urllib.parse"),
    MovedAttribute("uses_query", "urlparse", "urllib.parse"),
    MovedAttribute("uses_relative", "urlparse", "urllib.parse"),
]
for attr in _urllib_parse_moved_attributes:
    setattr(Module_six_moves_urllib_parse, attr.name, attr)
del attr

Module_six_moves_urllib_parse._moved_attributes = _urllib_parse_moved_attributes

_importer._add_module(Module_six_moves_urllib_parse(__name__ + ".moves.urllib_parse"),
                      "moves.urllib_parse", "moves.urllib.parse")


class Module_six_moves_urllib_error(_LazyModule):

    """Lazy loading of moved objects in six.moves.urllib_error"""


_urllib_error_moved_attributes = [
    MovedAttribute("URLError", "urllib2", "urllib.error"),
    MovedAttribute("HTTPError", "urllib2", "urllib.error"),
    MovedAttribute("ContentTooShortError", "urllib", "urllib.error"),
]
for attr in _urllib_error_moved_attributes:
    setattr(Module_six_moves_urllib_error, attr.name, attr)
del attr

Module_six_moves_urllib_error._moved_attributes = _urllib_error_moved_attributes

_importer._add_module(Module_six_moves_urllib_error(__name__ + ".moves.urllib.error"),
                      "moves.urllib_error", "moves.urllib.error")


class Module_six_moves_urllib_request(_LazyModule):

    """Lazy loading of moved objects in six.moves.urllib_request"""


_urllib_request_moved_attributes = [
    MovedAttribute("urlopen", "urllib2", "urllib.request"),
    MovedAttribute("install_opener", "urllib2", "urllib.request"),
    MovedAttribute("build_opener", "urllib2", "urllib.request"),
    MovedAttribute("pathname2url", "urllib", "urllib.request"),
    MovedAttribute("url2pathname", "urllib", "urllib.request"),
    MovedAttribute("getproxies", "urllib", "urllib.request"),
    MovedAttribute("Request", "urllib2", "urllib.request"),
    MovedAttribute("OpenerDirector", "urllib2", "urllib.request"),
    MovedAttribute("HTTPDefaultErrorHandler", "urllib2", "urllib.request"),
    MovedAttribute("HTTPRedirectHandler", "urllib2", "urllib.request"),
    MovedAttribute("HTTPCookieProcessor", "urllib2", "urllib.request"),
    MovedAttribute("ProxyHandler", "urllib2", "urllib.request"),
    MovedAttribute("BaseHandler", "urllib2", "urllib.request"),
    MovedAttribute("HTTPPasswordMgr", "urllib2", "urllib.request"),
    MovedAttribute("HTTPPasswordMgrWithDefaultRealm", "urllib2", "urllib.request"),
    MovedAttribute("AbstractBasicAuthHandler", "urllib2", "urllib.request"),
    MovedAttribute("HTTPBasicAuthHandler", "urllib2", "urllib.request"),
    MovedAttribute("ProxyBasicAuthHandler", "urllib2", "urllib.request"),
    MovedAttribute("AbstractDigestAuthHandler", "urllib2", "urllib.request"),
    MovedAttribute("HTTPDigestAuthHandler", "urllib2", "urllib.request"),
    MovedAttribute("ProxyDigestAuthHandler", "urllib2", "urllib.request"),
    MovedAttribute("HTTPHandler", "urllib2", "urllib.request"),
    MovedAttribute("HTTPSHandler", "urllib2", "urllib.request"),
    MovedAttribute("FileHandler", "urllib2", "urllib.request"),
    MovedAttribute("FTPHandler", "urllib2", "urllib.request"),
    MovedAttribute("CacheFTPHandler", "urllib2", "urllib.request"),
    MovedAttribute("UnknownHandler", "urllib2", "urllib.request"),
    MovedAttribute("HTTPErrorProcessor", "urllib2", "urllib.request"),
    MovedAttribute("urlretrieve", "urllib", "urllib.request"),
    MovedAttribute("urlcleanup", "urllib", "urllib.request"),
    MovedAttribute("proxy_bypass", "urllib", "urllib.request"),
    MovedAttribute("parse_http_list", "urllib2", "urllib.request"),
    MovedAttribute("parse_keqv_list", "urllib2", "urllib.request"),
]
if sys.version_info[:2] < (3, 14):
    _urllib_request_moved_attributes.extend(
        [
            MovedAttribute("URLopener", "urllib", "urllib.request"),
            MovedAttribute("FancyURLopener", "urllib", "urllib.request"),
        ]
    )
for attr in _urllib_request_moved_attributes:
    setattr(Module_six_moves_urllib_request, attr.name, attr)
del attr

Module_six_moves_urllib_request._moved_attributes = _urllib_request_moved_attributes

_importer._add_module(Module_six_moves_urllib_request(__name__ + ".moves.urllib.request"),
                      "moves.urllib_request", "moves.urllib.request")


class Module_six_moves_urllib_response(_LazyModule):

    """Lazy loading of moved objects in six.moves.urllib_response"""


_urllib_response_moved_attributes = [
    MovedAttribute("addbase", "urllib", "urllib.response"),
    MovedAttribute("addclosehook", "urllib", "urllib.response"),
    MovedAttribute("addinfo", "urllib", "urllib.response"),
    MovedAttribute("addinfourl", "urllib", "urllib.response"),
]
for attr in _urllib_response_moved_attributes:
    setattr(Module_six_moves_urllib_response, attr.name, attr)
del attr

Module_six_moves_urllib_response._moved_attributes = _urllib_response_moved_attributes

_importer._add_module(Module_six_moves_urllib_response(__name__ + ".moves.urllib.response"),
                      "moves.urllib_response", "moves.urllib.response")


class Module_six_moves_urllib_robotparser(_LazyModule):

    """Lazy loading of moved objects in six.moves.urllib_robotparser"""


_urllib_robotparser_moved_attributes = [
    MovedAttribute("RobotFileParser", "robotparser", "urllib.robotparser"),
]
for attr in _urllib_robotparser_moved_attributes:
    setattr(Module_six_moves_urllib_robotparser, attr.name, attr)
del attr

Module_six_moves_urllib_robotparser._moved_attributes = _urllib_robotparser_moved_attributes

_importer._add_module(Module_six_moves_urllib_robotparser(__name__ + ".moves.urllib.robotparser"),
                      "moves.urllib_robotparser", "moves.urllib.robotparser")


class Module_six_moves_urllib(types.ModuleType):

    """Create a six.moves.urllib namespace that resembles the Python 3 namespace"""
    __path__ = []  # mark as package
    parse = _importer._get_module("moves.urllib_parse")
    error = _importer._get_module("moves.urllib_error")
    request = _importer._get_module("moves.urllib_request")
    response = _importer._get_module("moves.urllib_response")
    robotparser = _importer._get_module("moves.urllib_robotparser")

    def __dir__(self):
        return ['parse', 'error', 'request', 'response', 'robotparser']

_importer._add_module(Module_six_moves_urllib(__name__ + ".moves.urllib"),
                      "moves.urllib")


def add_move(move):
    """Add an item to six.moves."""
    setattr(_MovedItems, move.name, move)


def remove_move(name):
    """Remove item from six.moves."""
    try:
        delattr(_MovedItems, name)
    except AttributeError:
        try:
            del moves.__dict__[name]
        except KeyError:
            raise AttributeError("no such move, %r" % (name,))


if PY3:
    _meth_func = "__func__"
    _meth_self = "__self__"

    _func_closure = "__closure__"
    _func_code = "__code__"
    _func_defaults = "__defaults__"
    _func_globals = "__globals__"
else:
    _meth_func = "im_func"
    _meth_self = "im_self"

    _func_closure = "func_closure"
    _func_code = "func_code"
    _func_defaults = "func_defaults"
    _func_globals = "func_globals"


try:
    advance_iterator = next
except NameError:
    def advance_iterator(it):
        return it.next()
next = advance_iterator


try:
    callable = callable
except NameError:
    def callable(obj):
        return any("__call__" in klass.__dict__ for klass in type(obj).__mro__)


if PY3:
    def get_unbound_function(unbound):
        return unbound

    create_bound_method = types.MethodType

    def create_unbound_method(func, cls):
        return func

    Iterator = object
else:
    def get_unbound_function(unbound):
        return unbound.im_func

    def create_bound_method(func, obj):
        return types.MethodType(func, obj, obj.__class__)

    def create_unbound_method(func, cls):
        return types.MethodType(func, None, cls)

    class Iterator(object):

        def next(self):
            return type(self).__next__(self)

    callable = callable
_add_doc(get_unbound_function,
         """Get the function out of a possibly unbound function""")


get_method_function = operator.attrgetter(_meth_func)
get_method_self = operator.attrgetter(_meth_self)
get_function_closure = operator.attrgetter(_func_closure)
get_function_code = operator.attrgetter(_func_code)
get_function_defaults = operator.attrgetter(_func_defaults)
get_function_globals = operator.attrgetter(_func_globals)


if PY3:
    def iterkeys(d, **kw):
        return iter(d.keys(**kw))

    def itervalues(d, **kw):
        return iter(d.values(**kw))

    def iteritems(d, **kw):
        return iter(d.items(**kw))

    def iterlists(d, **kw):
        return iter(d.lists(**kw))

    viewkeys = operator.methodcaller("keys")

    viewvalues = operator.methodcaller("values")

    viewitems = operator.methodcaller("items")
else:
    def iterkeys(d, **kw):
        return d.iterkeys(**kw)

    def itervalues(d, **kw):
        return d.itervalues(**kw)

    def iteritems(d, **kw):
        return d.iteritems(**kw)

    def iterlists(d, **kw):
        return d.iterlists(**kw)

    viewkeys = operator.methodcaller("viewkeys")

    viewvalues = operator.methodcaller("viewvalues")

    viewitems = operator.methodcaller("viewitems")

_add_doc(iterkeys, "Return an iterator over the keys of a dictionary.")
_add_doc(itervalues, "Return an iterator over the values of a dictionary.")
_add_doc(iteritems,
         "Return an iterator over the (key, value) pairs of a dictionary.")
_add_doc(iterlists,
         "Return an iterator over the (key, [values]) pairs of a dictionary.")


if PY3:
    def b(s):
        return s.encode("latin-1")

    def u(s):
        return s
    unichr = chr
    import struct
    int2byte = struct.Struct(">B").pack
    del struct
    byte2int = operator.itemgetter(0)
    indexbytes = operator.getitem
    iterbytes = iter
    import io
    StringIO = io.StringIO
    BytesIO = io.BytesIO
    del io
    _assertCountEqual = "assertCountEqual"
    if sys.version_info[1] <= 1:
        _assertRaisesRegex = "assertRaisesRegexp"
        _assertRegex = "assertRegexpMatches"
        _assertNotRegex = "assertNotRegexpMatches"
    else:
        _assertRaisesRegex = "assertRaisesRegex"
        _assertRegex = "assertRegex"
        _assertNotRegex = "assertNotRegex"
else:
    def b(s):
        return s
    # Workaround for standalone backslash

    def u(s):
        return unicode(s.replace(r'\\', r'\\\\'), "unicode_escape")
    unichr = unichr
    int2byte = chr

    def byte2int(bs):
        return ord(bs[0])

    def indexbytes(buf, i):
        return ord(buf[i])
    iterbytes = functools.partial(itertools.imap, ord)
    import StringIO
    StringIO = BytesIO = StringIO.StringIO
    _assertCountEqual = "assertItemsEqual"
    _assertRaisesRegex = "assertRaisesRegexp"
    _assertRegex = "assertRegexpMatches"
    _assertNotRegex = "assertNotRegexpMatches"
_add_doc(b, """Byte literal""")
_add_doc(u, """Text literal""")


def assertCountEqual(self, *args, **kwargs):
    return getattr(self, _assertCountEqual)(*args, **kwargs)


def assertRaisesRegex(self, *args, **kwargs):
    return getattr(self, _assertRaisesRegex)(*args, **kwargs)


def assertRegex(self, *args, **kwargs):
    return getattr(self, _assertRegex)(*args, **kwargs)


def assertNotRegex(self, *args, **kwargs):
    return getattr(self, _assertNotRegex)(*args, **kwargs)


if PY3:
    exec_ = getattr(moves.builtins, "exec")

    def reraise(tp, value, tb=None):
        try:
            if value is None:
                value = tp()
            if value.__traceback__ is not tb:
                raise value.with_traceback(tb)
            raise value
        finally:
            value = None
            tb = None

else:
    def exec_(_code_, _globs_=None, _locs_=None):
        """Execute code in a namespace."""
        if _globs_ is None:
            frame = sys._getframe(1)
            _globs_ = frame.f_globals
            if _locs_ is None:
                _locs_ = frame.f_locals
            del frame
        elif _locs_ is None:
            _locs_ = _globs_
        exec("""exec _code_ in _globs_, _locs_""")

    exec_("""def reraise(tp, value, tb=None):
    try:
        raise tp, value, tb
    finally:
        tb = None
""")


if sys.version_info[:2] > (3,):
    exec_("""def raise_from(value, from_value):
    try:
        raise value from from_value
    finally:
        value = None
""")
else:
    def raise_from(value, from_value):
        raise value


print_ = getattr(moves.builtins, "print", None)
if print_ is None:
    def print_(*args, **kwargs):
        """The new-style print function for Python 2.4 and 2.5."""
        fp = kwargs.pop("file", sys.stdout)
        if fp is None:
            return

        def write(data):
            if not isinstance(data, basestring):
                data = str(data)
            # If the file has an encoding, encode unicode with it.
            if (isinstance(fp, file) and
                    isinstance(data, unicode) and
                    fp.encoding is not None):
                errors = getattr(fp, "errors", None)
                if errors is None:
                    errors = "strict"
                data = data.encode(fp.encoding, errors)
            fp.write(data)
        want_unicode = False
        sep = kwargs.pop("sep", None)
        if sep is not None:
            if isinstance(sep, unicode):
                want_unicode = True
            elif not isinstance(sep, str):
                raise TypeError("sep must be None or a string")
        end = kwargs.pop("end", None)
        if end is not None:
            if isinstance(end, unicode):
                want_unicode = True
            elif not isinstance(end, str):
                raise TypeError("end must be None or a string")
        if kwargs:
            raise TypeError("invalid keyword arguments to print()")
        if not want_unicode:
            for arg in args:
                if isinstance(arg, unicode):
                    want_unicode = True
                    break
        if want_unicode:
            newline = unicode("\n")
            space = unicode(" ")
        else:
            newline = "\n"
            space = " "
        if sep is None:
            sep = space
        if end is None:
            end = newline
        for i, arg in enumerate(args):
            if i:
                write(sep)
            write(arg)
        write(end)
if sys.version_info[:2] < (3, 3):
    _print = print_

    def print_(*args, **kwargs):
        fp = kwargs.get("file", sys.stdout)
        flush = kwargs.pop("flush", False)
        _print(*args, **kwargs)
        if flush and fp is not None:
            fp.flush()

_add_doc(reraise, """Reraise an exception.""")

if sys.version_info[0:2] < (3, 4):
    # This does exactly the same what the :func:`py3:functools.update_wrapper`
    # function does on Python versions after 3.2. It sets the ``__wrapped__``
    # attribute on ``wrapper`` object and it doesn't raise an error if any of
    # the attributes mentioned in ``assigned`` and ``updated`` are missing on
    # ``wrapped`` object.
    def _update_wrapper(wrapper, wrapped,
                        assigned=functools.WRAPPER_ASSIGNMENTS,
                        updated=functools.WRAPPER_UPDATES):
        for attr in assigned:
            try:
                value = getattr(wrapped, attr)
            except AttributeError:
                continue
            else:
                setattr(wrapper, attr, value)
        for attr in updated:
            getattr(wrapper, attr).update(getattr(wrapped, attr, {}))
        wrapper.__wrapped__ = wrapped
        return wrapper
    _update_wrapper.__doc__ = functools.update_wrapper.__doc__

    def wraps(wrapped, assigned=functools.WRAPPER_ASSIGNMENTS,
              updated=functools.WRAPPER_UPDATES):
        return functools.partial(_update_wrapper, wrapped=wrapped,
                                 assigned=assigned, updated=updated)
    wraps.__doc__ = functools.wraps.__doc__

else:
    wraps = functools.wraps


def with_metaclass(meta, *bases):
    """Create a base class with a metaclass."""
    # This requires a bit of explanation: the basic idea is to make a dummy
    # metaclass for one level of class instantiation that replaces itself with
    # the actual metaclass.
    class metaclass(type):

        def __new__(cls, name, this_bases, d):
            if sys.version_info[:2] >= (3, 7):
                # This version introduced PEP 560 that requires a bit
                # of extra care (we mimic what is done by __build_class__).
                resolved_bases = types.resolve_bases(bases)
                if resolved_bases is not bases:
                    d['__orig_bases__'] = bases
            else:
                resolved_bases = bases
            return meta(name, resolved_bases, d)

        @classmethod
        def __prepare__(cls, name, this_bases):
            return meta.__prepare__(name, bases)
    return type.__new__(metaclass, 'temporary_class', (), {})


def add_metaclass(metaclass):
    """Class decorator for creating a class with a metaclass."""
    def wrapper(cls):
        orig_vars = cls.__dict__.copy()
        slots = orig_vars.get('__slots__')
        if slots is not None:
            if isinstance(slots, str):
                slots = [slots]
            for slots_var in slots:
                orig_vars.pop(slots_var)
        orig_vars.pop('__dict__', None)
        orig_vars.pop('__weakref__', None)
        if hasattr(cls, '__qualname__'):
            orig_vars['__qualname__'] = cls.__qualname__
        return metaclass(cls.__name__, cls.__bases__, orig_vars)
    return wrapper


def ensure_binary(s, encoding='utf-8', errors='strict'):
    """Coerce **s** to six.binary_type.

    For Python 2:
      - `unicode` -> encoded to `str`
      - `str` -> `str`

    For Python 3:
      - `str` -> encoded to `bytes`
      - `bytes` -> `bytes`
    """
    if isinstance(s, binary_type):
        return s
    if isinstance(s, text_type):
        return s.encode(encoding, errors)
    raise TypeError("not expecting type '%s'" % type(s))


def ensure_str(s, encoding='utf-8', errors='strict'):
    """Coerce *s* to `str`.

    For Python 2:
      - `unicode` -> encoded to `str`
      - `str` -> `str`

    For Python 3:
      - `str` -> `str`
      - `bytes` -> decoded to `str`
    """
    # Optimization: Fast return for the common case.
    if type(s) is str:
        return s
    if PY2 and isinstance(s, text_type):
        return s.encode(encoding, errors)
    elif PY3 and isinstance(s, binary_type):
        return s.decode(encoding, errors)
    elif not isinstance(s, (text_type, binary_type)):
        raise TypeError("not expecting type '%s'" % type(s))
    return s


def ensure_text(s, encoding='utf-8', errors='strict'):
    """Coerce *s* to six.text_type.

    For Python 2:
      - `unicode` -> `unicode`
      - `str` -> `unicode`

    For Python 3:
      - `str` -> `str`
      - `bytes` -> decoded to `str`
    """
    if isinstance(s, binary_type):
        return s.decode(encoding, errors)
    elif isinstance(s, text_type):
        return s
    else:
        raise TypeError("not expecting type '%s'" % type(s))


def python_2_unicode_compatible(klass):
    """
    A class decorator that defines __unicode__ and __str__ methods under Python 2.
    Under Python 3 it does nothing.

    To support Python 2 and 3 with a single code base, define a __str__ method
    returning text and apply this decorator to the class.
    """
    if PY2:
        if '__str__' not in klass.__dict__:
            raise ValueError("@python_2_unicode_compatible cannot be applied "
                             "to %s because it doesn't define __str__()." %
                             klass.__name__)
        klass.__unicode__ = klass.__str__
        klass.__str__ = lambda self: self.__unicode__().encode('utf-8')
    return klass


# Complete the moves implementation.
# This code is at the end of this module to speed up module loading.
# Turn this module into a package.
__path__ = []  # required for PEP 302 and PEP 451
__package__ = __name__  # see PEP 366 @ReservedAssignment
if globals().get("__spec__") is not None:
    __spec__.submodule_search_locations = []  # PEP 451 @UndefinedVariable
# Remove other six meta path importers, since they cause problems. This can
# happen if six is removed from sys.modules and then reloaded. (Setuptools does
# this for some reason.)
if sys.meta_path:
    for i, importer in enumerate(sys.meta_path):
        # Here's some real nastiness: Another "instance" of the six module might
        # be floating around. Therefore, we can't use isinstance() to check for
        # the six meta path importer, since the other six instance will have
        # inserted an importer with different class.
        if (type(importer).__name__ == "_SixMetaPathImporter" and
                importer.name == __name__):
            del sys.meta_path[i]
            break
    del i, importer
# Finally, add the importer to the meta path import hook.
sys.meta_path.append(_importer)


# ==== vendored transitions.__init__ ====
# transitions
# -----------
#
# A lightweight, object-oriented state machine implementation in Python. Compatible with Python 2.7+ and 3.0+.

# (vendored) from .version imported inline
# (vendored) from .core imported inline

__copyright__ = "Copyright (c) 2024 Tal Yarkoni, Alexander Neumann"
__license__ = "MIT"
__summary__ = "A lightweight, object-oriented finite state machine implementation in Python with many extensions"
__uri__ = "https://github.com/pytransitions/transitions"


# ==== vendored transitions.core ====
# transitions.core
#     ----------------
#
#     This module contains the central parts of transitions which are the state machine logic, state
#     and transition concepts.

try:
    from builtins import object
except ImportError:  # pragma: no cover
    # python2
    pass

try:
    # Enums are supported for Python 3.4+ and Python 2.7 with enum34 package installed
    from enum import Enum, EnumMeta
except ImportError:  # pragma: no cover
    # If enum is not available, create dummy classes for type checks
    class Enum:  # type:ignore
        """This is just an Enum stub for Python 2 and Python 3.3 and before without Enum support."""

    class EnumMeta:  # type:ignore
        """This is just an EnumMeta stub for Python 2 and Python 3.3 and before without Enum support."""

import inspect
import itertools
import logging
import warnings

from collections import OrderedDict, defaultdict, deque
from functools import partial

_LOGGER = logging.getLogger(__name__)
_LOGGER.addHandler(logging.NullHandler())

warnings.filterwarnings(action='default', message=r".*transitions version.*", category=DeprecationWarning)


def listify(obj):
    """Wraps a passed object into a list in case it has not been a list, tuple before.
    Returns an empty list in case ``obj`` is None.
    Args:
        obj: instance to be converted into a list.
    Returns:
        list: May also return a tuple in case ``obj`` has been a tuple before.
    """
    if obj is None:
        return []

    try:
        return obj if isinstance(obj, (list, tuple, EnumMeta)) else [obj]
    except ReferenceError:
        # obj is an empty weakref
        return [obj]


def _prep_ordered_arg(desired_length, arguments=None):
    """Ensure list of arguments passed to add_ordered_transitions has the proper length.
    Expands the given arguments and apply same condition, callback
    to all transitions if only one has been given.

    Args:
        desired_length (int): The size of the resulting list
        arguments (optional[str, reference or list]): Parameters to be expanded.
    Returns:
        list: Parameter sets with the desired length.
    """
    arguments = listify(arguments) if arguments is not None else [None]
    if len(arguments) != desired_length and len(arguments) != 1:
        raise ValueError("Argument length must be either 1 or the same length as "
                         "the number of transitions.")
    if len(arguments) == 1:
        return arguments * desired_length
    return arguments


class State(object):
    """A persistent representation of a state managed by a ``Machine``.

    Attributes:
        name (str): State name which is also assigned to the model(s).
        on_enter (list): Callbacks executed when a state is entered.
        on_exit (list): Callbacks executed when a state is exited.
        ignore_invalid_triggers (bool): Indicates if unhandled/invalid triggers should raise an exception.
    """

    # A list of dynamic methods which can be resolved by a ``Machine`` instance for convenience functions.
    # Dynamic methods for states must always start with `on_`!
    dynamic_methods = ['on_enter', 'on_exit']

    def __init__(self, name, on_enter=None, on_exit=None,
                 ignore_invalid_triggers=None, final=False):
        """
        Args:
            name (str or Enum): The name of the state
            on_enter (str or list): Optional callable(s) to trigger when a
                state is entered. Can be either a string providing the name of
                a callable, or a list of strings.
            on_exit (str or list): Optional callable(s) to trigger when a
                state is exited. Can be either a string providing the name of a
                callable, or a list of strings.
            ignore_invalid_triggers (Boolean): Optional flag to indicate if
                unhandled/invalid triggers should raise an exception

        """
        self._name = name
        self.final = final
        self.ignore_invalid_triggers = ignore_invalid_triggers
        self.on_enter = listify(on_enter) if on_enter else []
        self.on_exit = listify(on_exit) if on_exit else []

    @property
    def name(self):
        """The name of the state."""
        if isinstance(self._name, Enum):
            return self._name.name
        return self._name

    @property
    def value(self):
        """The state's value. For string states this will be equivalent to the name attribute."""
        return self._name

    def enter(self, event_data):
        """Triggered when a state is entered."""
        _LOGGER.debug("%sEntering state %s. Processing callbacks...", event_data.machine.name, self.name)
        event_data.machine.callbacks(self.on_enter, event_data)
        _LOGGER.info("%sFinished processing state %s enter callbacks.", event_data.machine.name, self.name)

    def exit(self, event_data):
        """Triggered when a state is exited."""
        _LOGGER.debug("%sExiting state %s. Processing callbacks...", event_data.machine.name, self.name)
        event_data.machine.callbacks(self.on_exit, event_data)
        _LOGGER.info("%sFinished processing state %s exit callbacks.", event_data.machine.name, self.name)

    def add_callback(self, trigger, func):
        """Add a new enter or exit callback.
        Args:
            trigger (str): The type of triggering event. Must be one of
                'enter' or 'exit'.
            func (str): The name of the callback function.
        """
        callback_list = getattr(self, 'on_' + trigger)
        callback_list.append(func)

    def __repr__(self):
        return "<%s('%s')@%s>" % (type(self).__name__, self.name, id(self))


class Condition(object):
    """A helper class to call condition checks in the intended way.

    Attributes:
        func (str or callable): The function to call for the condition check
        target (bool): Indicates the target state--i.e., when True,
                the condition-checking callback should return True to pass,
                and when False, the callback should return False to pass.
    """

    def __init__(self, func, target=True):
        """
        Args:
            func (str or callable): Name of the condition-checking callable
            target (bool): Indicates the target state--i.e., when True,
                the condition-checking callback should return True to pass,
                and when False, the callback should return False to pass.
        Notes:
            This class should not be initialized or called from outside a
            Transition instance, and exists at module level (rather than
            nesting under the transition class) only because of a bug in
            dill that prevents serialization under Python 2.7.
        """
        self.func = func
        self.target = target

    def check(self, event_data):
        """Check whether the condition passes.
        Args:
            event_data (EventData): An EventData instance to pass to the
                condition (if event sending is enabled) or to extract arguments
                from (if event sending is disabled). Also contains the data
                model attached to the current machine which is used to invoke
                the condition.
        """
        predicate = event_data.machine.resolve_callable(self.func, event_data)
        if event_data.machine.send_event:
            return predicate(event_data) == self.target
        return predicate(*event_data.args, **event_data.kwargs) == self.target

    def __repr__(self):
        return "<%s(%s)@%s>" % (type(self).__name__, self.func, id(self))


class Transition(object):
    """Representation of a transition managed by a ``Machine`` instance.

    Attributes:
        source (str): Source state of the transition.
        dest (str): Destination state of the transition.
        prepare (list): Callbacks executed before conditions checks.
        conditions (list): Callbacks evaluated to determine if
            the transition should be executed.
        before (list): Callbacks executed before the transition is executed
            but only if condition checks have been successful.
        after (list): Callbacks executed after the transition is executed
            but only if condition checks have been successful.
    """

    dynamic_methods = ['before', 'after', 'prepare']
    """ A list of dynamic methods which can be resolved by a ``Machine`` instance for convenience functions. """
    condition_cls = Condition
    """ The class used to wrap condition checks. Can be replaced to alter condition resolution behaviour
        (e.g. OR instead of AND for 'conditions' or AND instead of OR for 'unless') """

    def __init__(self, source, dest, conditions=None, unless=None, before=None,
                 after=None, prepare=None):
        """
        Args:
            source (str): The name of the source State.
            dest (str): The name of the destination State.
            conditions (optional[str, callable or list]): Condition(s) that must pass in order for
                the transition to take place. Either a string providing the
                name of a callable, or a list of callables. For the transition
                to occur, ALL callables must return True.
            unless (optional[str, callable or list]): Condition(s) that must return False in order
                for the transition to occur. Behaves just like conditions arg
                otherwise.
            before (optional[str, callable or list]): callbacks to trigger before the
                transition.
            after (optional[str, callable or list]): callbacks to trigger after the transition.
            prepare (optional[str, callable or list]): callbacks to trigger before conditions are checked
        """
        self.source = source
        self.dest = dest
        self.prepare = [] if prepare is None else listify(prepare)
        self.before = [] if before is None else listify(before)
        self.after = [] if after is None else listify(after)

        self.conditions = []
        if conditions is not None:
            for cond in listify(conditions):
                self.conditions.append(self.condition_cls(cond))
        if unless is not None:
            for cond in listify(unless):
                self.conditions.append(self.condition_cls(cond, target=False))

    def _eval_conditions(self, event_data):
        for cond in self.conditions:
            if not cond.check(event_data):
                _LOGGER.debug("%sTransition condition failed: %s() does not return %s. Transition halted.",
                              event_data.machine.name, cond.func, cond.target)
                return False
        return True

    def execute(self, event_data):
        """Execute the transition.
        Args:
            event_data: An instance of class EventData.
        Returns: boolean indicating whether the transition was
            successfully executed (True if successful, False if not).
        """
        _LOGGER.debug("%sInitiating transition from state %s to state %s...",
                      event_data.machine.name, self.source, self.dest)

        event_data.machine.callbacks(self.prepare, event_data)
        _LOGGER.debug("%sExecuted callbacks before conditions.", event_data.machine.name)

        if not self._eval_conditions(event_data):
            return False

        event_data.machine.callbacks(itertools.chain(event_data.machine.before_state_change, self.before), event_data)
        _LOGGER.debug("%sExecuted callback before transition.", event_data.machine.name)

        if self.dest:  # if self.dest is None this is an internal transition with no actual state change
            self._change_state(event_data)

        event_data.machine.callbacks(itertools.chain(self.after, event_data.machine.after_state_change), event_data)
        _LOGGER.debug("%sExecuted callback after transition.", event_data.machine.name)
        return True

    def _change_state(self, event_data):
        event_data.machine.get_state(self.source).exit(event_data)
        event_data.machine.set_state(self.dest, event_data.model)
        event_data.update(getattr(event_data.model, event_data.machine.model_attribute))
        dest = event_data.machine.get_state(self.dest)
        dest.enter(event_data)
        if dest.final:
            event_data.machine.callbacks(event_data.machine.on_final, event_data)

    def add_callback(self, trigger, func):
        """Add a new before, after, or prepare callback.
        Args:
            trigger (str): The type of triggering event. Must be one of
                'before', 'after' or 'prepare'.
            func (str or callable): The name of the callback function or a callable.
        """
        callback_list = getattr(self, trigger)
        callback_list.append(func)

    def __repr__(self):
        return "<%s('%s', '%s')@%s>" % (type(self).__name__,
                                        self.source, self.dest, id(self))


class EventData(object):
    """Collection of relevant data related to the ongoing transition attempt.

    Attributes:
        state (State): The State from which the Event was triggered.
        event (Event): The triggering Event.
        machine (Machine): The current Machine instance.
        model (object): The model/object the machine is bound to.
        args (list): Optional positional arguments from trigger method
            to store internally for possible later use.
        kwargs (dict): Optional keyword arguments from trigger method
            to store internally for possible later use.
        transition (Transition): Currently active transition. Will be assigned during triggering.
        error (Exception): In case a triggered event causes an Error, it is assigned here and passed on.
        result (bool): True in case a transition has been successful, False otherwise.
    """

    def __init__(self, state, event, machine, model, args, kwargs):
        """
        Args:
            state (State): The State from which the Event was triggered.
            event (Event): The triggering Event.
            machine (Machine): The current Machine instance.
            model (object): The model/object the machine is bound to.
            args (tuple): Optional positional arguments from trigger method
                to store internally for possible later use.
            kwargs (dict): Optional keyword arguments from trigger method
                to store internally for possible later use.
        """
        self.state = state
        self.event = event
        self.machine = machine
        self.model = model
        self.args = args
        self.kwargs = kwargs
        self.transition = None
        self.error = None
        self.result = False

    def update(self, state):
        """Updates the EventData object with the passed state.

        Attributes:
            state (State, str or Enum): The state object, enum member or string to assign to EventData.
        """

        if not isinstance(state, State):
            self.state = self.machine.get_state(state)

    def __repr__(self):
        return "<%s(%s, %s, %s)@%s>" % (type(self).__name__, self.event, self.state,
                                        getattr(self, 'transition'), id(self))


class Event(object):
    """A collection of transitions assigned to the same trigger

    """

    def __init__(self, name, machine):
        """
        Args:
            name (str): The name of the event, which is also the name of the
                triggering callable (e.g., 'advance' implies an advance()
                method).
            machine (Machine): The current Machine instance.
        """
        self.name = name
        self.machine = machine
        self.transitions = defaultdict(list)

    def add_transition(self, transition):
        """Add a transition to the list of potential transitions.
        Args:
            transition (Transition): The Transition instance to add to the
                list.
        """
        self.transitions[transition.source].append(transition)

    def trigger(self, model, *args, **kwargs):
        """Executes all transitions that match the current state,
        halting as soon as one successfully completes. More precisely, it prepares a partial
        of the internal ``_trigger`` function, passes this to ``Machine._process``.
        It is up to the machine's configuration of the Event whether processing happens queued (sequentially) or
        whether further Events are processed as they occur.
        Args:
            model (object): The currently processed model
            args and kwargs: Optional positional or named arguments that will
                be passed onto the EventData object, enabling arbitrary state
                information to be passed on to downstream triggered functions.
        Returns: boolean indicating whether a transition was
            successfully executed (True if successful, False if not).
        """
        func = partial(self._trigger, EventData(None, self, self.machine, model, args=args, kwargs=kwargs))
        # pylint: disable=protected-access
        # noinspection PyProtectedMember
        # Machine._process should not be called somewhere else. That's why it should not be exposed
        # to Machine users.
        return self.machine._process(func)

    def _trigger(self, event_data):
        """Internal trigger function called by the ``Machine`` instance. This should not
        be called directly but via the public method ``Machine.process``.
        Args:
            event_data (EventData): The currently processed event. State, result and (potentially) error might be
            overridden.
        Returns: boolean indicating whether a transition was
            successfully executed (True if successful, False if not).
        """
        event_data.state = self.machine.get_model_state(event_data.model)
        try:
            if self._is_valid_source(event_data.state):
                self._process(event_data)
        except BaseException as err:  # pylint: disable=broad-except; Exception will be handled elsewhere
            event_data.error = err
            if self.machine.on_exception:
                self.machine.callbacks(self.machine.on_exception, event_data)
            else:
                raise
        finally:
            try:
                self.machine.callbacks(self.machine.finalize_event, event_data)
                _LOGGER.debug("%sExecuted machine finalize callbacks", self.machine.name)
            except BaseException as err:  # pylint: disable=broad-except; Exception will be handled elsewhere
                _LOGGER.error("%sWhile executing finalize callbacks a %s occurred: %s.",
                              self.machine.name,
                              type(err).__name__,
                              str(err))
        return event_data.result

    def _process(self, event_data):
        self.machine.callbacks(self.machine.prepare_event, event_data)
        _LOGGER.debug("%sExecuted machine preparation callbacks before conditions.", self.machine.name)
        for trans in self.transitions[event_data.state.name]:
            event_data.transition = trans
            if trans.execute(event_data):
                event_data.result = True
                break

    def _is_valid_source(self, state):
        if state.name not in self.transitions:
            msg = "%sCan't trigger event %s from state %s!" % (self.machine.name, self.name,
                                                               state.name)
            ignore = state.ignore_invalid_triggers if state.ignore_invalid_triggers is not None \
                else self.machine.ignore_invalid_triggers
            if ignore:
                _LOGGER.warning(msg)
                return False
            raise MachineError(msg)
        return True

    def __repr__(self):
        return "<%s('%s')@%s>" % (type(self).__name__, self.name, id(self))

    def add_callback(self, trigger, func):
        """Add a new before or after callback to all available transitions.
        Args:
            trigger (str): The type of triggering event. Must be one of
                'before', 'after' or 'prepare'.
            func (str): The name of the callback function.
        """
        for trans in itertools.chain(*self.transitions.values()):
            trans.add_callback(trigger, func)


class Machine(object):
    """Machine manages states, transitions and models. In case it is initialized without a specific model
    (or specifically no model), it will also act as a model itself. Machine takes also care of decorating
    models with conveniences functions related to added transitions and states during runtime.

    Attributes:
        states (OrderedDict): Collection of all registered states.
        events (dict): Collection of transitions ordered by trigger/event.
        models (list): List of models attached to the machine.
        initial (str): Name of the initial state for new models.
        prepare_event (list): Callbacks executed when an event is triggered.
        before_state_change (list): Callbacks executed after condition checks but before transition is conducted.
            Callbacks will be executed BEFORE the custom callbacks assigned to the transition.
        after_state_change (list): Callbacks executed after the transition has been conducted.
            Callbacks will be executed AFTER the custom callbacks assigned to the transition.
        finalize_event (list): Callbacks will be executed after all transitions callbacks have been executed.
            Callbacks mentioned here will also be called if a transition or condition check raised an error.
        queued (bool): Whether transitions in callbacks should be executed immediately (False) or sequentially.
        send_event (bool): When True, any arguments passed to trigger methods will be wrapped in an EventData
            object, allowing indirect and encapsulated access to data. When False, all positional and keyword
            arguments will be passed directly to all callback methods.
        auto_transitions (bool):  When True (default), every state will automatically have an associated
            to_{state}() convenience trigger in the base model.
        ignore_invalid_triggers (bool): When True, any calls to trigger methods that are not valid for the
            present state (e.g., calling an a_to_b() trigger when the current state is c) will be silently
            ignored rather than raising an invalid transition exception.
        name (str): Name of the ``Machine`` instance mainly used for easier log message distinction.
    """

    separator = '_'  # separates callback type from state/transition name
    wildcard_all = '*'  # will be expanded to ALL states
    wildcard_same = '='  # will be expanded to source state
    state_cls = State
    transition_cls = Transition
    event_cls = Event
    self_literal = 'self'

    def __init__(self, model=self_literal, states=None, initial='initial', transitions=None,
                 send_event=False, auto_transitions=True,
                 ordered_transitions=False, ignore_invalid_triggers=None,
                 before_state_change=None, after_state_change=None, name=None,
                 queued=False, prepare_event=None, finalize_event=None, model_attribute='state', model_override=False,
                 on_exception=None, on_final=None, **kwargs):
        """
        Args:
            model (object or list): The object(s) whose states we want to manage. If set to `Machine.self_literal`
                (default value), the current Machine instance will be used as the model (i.e., all
                triggering events will be attached to the Machine itself). Note that an empty list
                is treated like no model.
            states (list or Enum): A list or enumeration of valid states. Each list element can be either a
                string, an enum member or a State instance. If string or enum member, a new generic State
                instance will be created that is named according to the string or enum member's name.
            initial (str, Enum or State): The initial state of the passed model[s].
            transitions (list): An optional list of transitions. Each element
                is a dictionary of named arguments to be passed onto the
                Transition initializer.
            send_event (boolean): When True, any arguments passed to trigger
                methods will be wrapped in an EventData object, allowing
                indirect and encapsulated access to data. When False, all
                positional and keyword arguments will be passed directly to all
                callback methods.
            auto_transitions (boolean): When True (default), every state will
                automatically have an associated to_{state}() convenience
                trigger in the base model.
            ordered_transitions (boolean): Convenience argument that calls
                add_ordered_transitions() at the end of initialization if set
                to True.
            ignore_invalid_triggers: when True, any calls to trigger methods
                that are not valid for the present state (e.g., calling an
                a_to_b() trigger when the current state is c) will be silently
                ignored rather than raising an invalid transition exception.
            before_state_change: A callable called on every change state before
                the transition happened. It receives the very same args as normal
                callbacks.
            after_state_change: A callable called on every change state after
                the transition happened. It receives the very same args as normal
                callbacks.
            name: If a name is set, it will be used as a prefix for logger output
            queued (boolean): When True, processes transitions sequentially. A trigger
                executed in a state callback function will be queued and executed later.
                Due to the nature of the queued processing, all transitions will
                _always_ return True since conditional checks cannot be conducted at queueing time.
            prepare_event: A callable called on for before possible transitions will be processed.
                It receives the very same args as normal callbacks.
            finalize_event: A callable called on for each triggered event after transitions have been processed.
                This is also called when a transition raises an exception.
            on_exception: A callable called when an event raises an exception. If not set,
                the exception will be raised instead.

            **kwargs additional arguments passed to next class in MRO. This can be ignored in most cases.
        """

        # calling super in case `Machine` is used as a mix in
        # all keyword arguments should be consumed by now if this is not the case
        try:
            super(Machine, self).__init__(**kwargs)
        except TypeError as err:
            raise ValueError('Passing arguments {0} caused an inheritance error: {1}'.format(kwargs.keys(), err))

        # initialize protected attributes first
        self._queued = queued
        self._transition_queue = deque()
        self._before_state_change = []
        self._after_state_change = []
        self._prepare_event = []
        self._finalize_event = []
        self._on_exception = []
        self._on_final = []
        self._initial = None

        self.states = OrderedDict()
        self.events = OrderedDict()
        self.send_event = send_event
        self.auto_transitions = auto_transitions
        self.ignore_invalid_triggers = ignore_invalid_triggers
        self.prepare_event = prepare_event
        self.before_state_change = before_state_change
        self.after_state_change = after_state_change
        self.finalize_event = finalize_event
        self.on_exception = on_exception
        self.on_final = on_final
        self.name = name + ": " if name is not None else ""
        self.model_attribute = model_attribute
        self.model_override = model_override

        self.models = []

        if states is not None:
            self.add_states(states)

        if initial is not None:
            self.initial = initial

        if transitions is not None:
            self.add_transitions(transitions)

        if ordered_transitions:
            self.add_ordered_transitions()

        if model:
            self.add_model(model)

    def add_model(self, model, initial=None):
        """Register a model with the state machine, initializing triggers and callbacks."""
        models = listify(model)

        if initial is None:
            if self.initial is None:
                raise ValueError("No initial state configured for machine, must specify when adding model.")
            initial = self.initial

        for mod in models:
            mod = self if mod is self.self_literal else mod
            if mod not in self.models:
                self._checked_assignment(mod, 'trigger', partial(self._get_trigger, mod))
                self._checked_assignment(mod, 'may_trigger', partial(self._can_trigger, mod))

                for trigger in self.events:
                    self._add_trigger_to_model(trigger, mod)

                for state in self.states.values():
                    self._add_model_to_state(state, mod)

                self.set_state(initial, model=mod)
                self.models.append(mod)

    def remove_model(self, model):
        """Remove a model from the state machine. The model will still contain all previously added triggers
        and callbacks, but will not receive updates when states or transitions are added to the Machine.
        If an event queue is used, all queued events of that model will be removed."""
        models = listify(model)

        for mod in models:
            self.models.remove(mod)
        if len(self._transition_queue) > 0:
            # the first element of the list is currently executed. Keeping it for further Machine._process(ing)
            self._transition_queue = deque(
                [self._transition_queue[0]] + [e for e in self._transition_queue if e.args[0].model not in models])

    @classmethod
    def _create_transition(cls, *args, **kwargs):
        return cls.transition_cls(*args, **kwargs)

    @classmethod
    def _create_event(cls, *args, **kwargs):
        return cls.event_cls(*args, **kwargs)

    @classmethod
    def _create_state(cls, *args, **kwargs):
        return cls.state_cls(*args, **kwargs)

    @property
    def initial(self):
        """Return the initial state."""
        return self._initial

    @initial.setter
    def initial(self, value):
        if isinstance(value, State):
            if value.name not in self.states:
                self.add_states(value)
            else:
                _ = self._has_state(value, raise_error=True)
            self._initial = value.name
        else:
            state_name = value.name if isinstance(value, Enum) else value
            if state_name not in self.states:
                self.add_states(state_name)
            self._initial = state_name

    @property
    def has_queue(self):
        """Return boolean indicating if machine has queue or not"""
        return self._queued

    @property
    def model(self):
        """List of models attached to the machine. For backwards compatibility, the property will
        return the model instance itself instead of the underlying list  if there is only one attached
        to the machine.
        """
        if len(self.models) == 1:
            return self.models[0]
        return self.models

    @property
    def before_state_change(self):
        """Callbacks executed after condition checks but before transition is conducted.
        Callbacks will be executed BEFORE the custom callbacks assigned to the transition."""
        return self._before_state_change

    # this should make sure that _before_state_change is always a list
    @before_state_change.setter
    def before_state_change(self, value):
        self._before_state_change = listify(value)

    @property
    def after_state_change(self):
        """Callbacks executed after the transition has been conducted.
        Callbacks will be executed AFTER the custom callbacks assigned to the transition."""
        return self._after_state_change

    # this should make sure that _after_state_change is always a list
    @after_state_change.setter
    def after_state_change(self, value):
        self._after_state_change = listify(value)

    @property
    def prepare_event(self):
        """Callbacks executed when an event is triggered."""
        return self._prepare_event

    # this should make sure that prepare_event is always a list
    @prepare_event.setter
    def prepare_event(self, value):
        self._prepare_event = listify(value)

    @property
    def finalize_event(self):
        """Callbacks will be executed after all transitions callbacks have been executed.
        Callbacks mentioned here will also be called if a transition or condition check raised an error."""
        return self._finalize_event

    # this should make sure that finalize_event is always a list
    @finalize_event.setter
    def finalize_event(self, value):
        self._finalize_event = listify(value)

    @property
    def on_exception(self):
        """Callbacks will be executed when an Event raises an Exception."""
        return self._on_exception

    # this should make sure that finalize_event is always a list
    @on_exception.setter
    def on_exception(self, value):
        self._on_exception = listify(value)

    @property
    def on_final(self):
        """Callbacks will be executed when the reached state is tagged with 'final'"""
        return self._on_final

    # this should make sure that finalize_event is always a list
    @on_final.setter
    def on_final(self, value):
        self._on_final = listify(value)

    def get_state(self, state):
        """Return the State instance with the passed name."""
        if isinstance(state, Enum):
            state = state.name
        if state not in self.states:
            raise ValueError("State '%s' is not a registered state." % state)
        return self.states[state]

    # In theory this function could be static. This however causes some issues related to inheritance and
    # pickling down the chain.
    def is_state(self, state, model):
        """Check whether the current state matches the named state. This function is not called directly
            but assigned as partials to model instances (e.g. is_A -> partial(_is_state, 'A', model)).
        Args:
            state (str or Enum): name of the checked state or Enum
            model: model to be checked
        Returns:
            bool: Whether the model's current state is state.
        """
        return getattr(model, self.model_attribute) == state

    def get_model_state(self, model):
        """
            Get the state of a model
        Args:
            model (object): the stateful model
        Returns:
            State: The State object related to the model's state
        """
        return self.get_state(getattr(model, self.model_attribute))

    def set_state(self, state, model=None):
        """
            Set the current state.
        Args:
            state (str or Enum or State): value of state to be set
            model (optional[object]): targeted model; if not set, all models will be set to 'state'
        """
        if not isinstance(state, State):
            state = self.get_state(state)
        models = self.models if model is None else listify(model)

        for mod in models:
            setattr(mod, self.model_attribute, state.value)

    def add_state(self, states, on_enter=None, on_exit=None, ignore_invalid_triggers=None, **kwargs):
        """Alias for add_states."""
        self.add_states(states=states, on_enter=on_enter, on_exit=on_exit,
                        ignore_invalid_triggers=ignore_invalid_triggers, **kwargs)

    def add_states(self, states, on_enter=None, on_exit=None,
                   ignore_invalid_triggers=None, **kwargs):
        """Add new state(s).
        Args:
            states (list, str, dict, Enum or State): a list, a State instance, the
                name of a new state, an enumeration (member) or a dict with keywords to pass on to the
                State initializer. If a list, each element can be a string, State or enumeration member.
            on_enter (str or list): callbacks to trigger when the state is
                entered. Only valid if first argument is string.
            on_exit (str or list): callbacks to trigger when the state is
                exited. Only valid if first argument is string.
            ignore_invalid_triggers: when True, any calls to trigger methods
                that are not valid for the present state (e.g., calling an
                a_to_b() trigger when the current state is c) will be silently
                ignored rather than raising an invalid transition exception.
                Note that this argument takes precedence over the same
                argument defined at the Machine level, and is in turn
                overridden by any ignore_invalid_triggers explicitly
                passed in an individual state's initialization arguments.

            **kwargs additional keyword arguments used by state mixins.
        """

        ignore = ignore_invalid_triggers
        if ignore is None:
            ignore = self.ignore_invalid_triggers

        states = listify(states)

        for state in states:
            if isinstance(state, (string_types, Enum)):
                state = self._create_state(
                    state, on_enter=on_enter, on_exit=on_exit,
                    ignore_invalid_triggers=ignore, **kwargs)
            elif isinstance(state, dict):
                if 'ignore_invalid_triggers' not in state:
                    state['ignore_invalid_triggers'] = ignore
                state = self._create_state(**state)
            self.states[state.name] = state
            for model in self.models:
                self._add_model_to_state(state, model)
            if self.auto_transitions:
                for a_state in self.states.keys():
                    # add all states as sources to auto transitions 'to_<state>' with dest <state>
                    if a_state == state.name:
                        if self.model_attribute == 'state':
                            method_name = 'to_%s' % a_state
                        else:
                            method_name = 'to_%s_%s' % (self.model_attribute, a_state)
                        self.add_transition(method_name, self.wildcard_all, a_state)

                    # add auto transition with source <state> to <a_state>
                    else:
                        if self.model_attribute == 'state':
                            method_name = 'to_%s' % a_state
                        else:
                            method_name = 'to_%s_%s' % (self.model_attribute, a_state)
                        self.add_transition(method_name, state.name, a_state)

    def _add_model_to_state(self, state, model):
        # Add convenience function 'is_<state_name>' (e.g. 'is_A') to the model.
        # When model_attribute has been customized, add 'is_<model_attribute>_<state_name>' instead
        # to potentially support multiple states on one model (e.g. 'is_custom_state_A' and 'is_my_state_B').

        func = partial(self.is_state, state.value, model)
        if self.model_attribute == 'state':
            method_name = 'is_%s' % state.name
        else:
            method_name = 'is_%s_%s' % (self.model_attribute, state.name)
        self._checked_assignment(model, method_name, func)

        # Add dynamic method callbacks (enter/exit) if there are existing bound methods in the model
        # except if they are already mentioned in 'on_enter/exit' of the defined state
        for callback in self.state_cls.dynamic_methods:
            method = "{0}_{1}".format(callback, state.name)
            if hasattr(model, method) and inspect.ismethod(getattr(model, method)) and \
                    method not in getattr(state, callback):
                state.add_callback(callback[3:], method)

    def _checked_assignment(self, model, name, func):
        bound_func = getattr(model, name, None)
        if (bound_func is None) ^ self.model_override:
            setattr(model, name, func)
        else:
            _LOGGER.warning("%sSkip binding of '%s' to model due to model override policy.", self.name, name)

    def _can_trigger(self, model, trigger, *args, **kwargs):
        state = self.get_model_state(model)
        event_data = EventData(state, Event(name=trigger, machine=self), self, model, args, kwargs)

        for trigger_name in self.get_triggers(state):
            if trigger_name != trigger:
                continue
            for transition in self.events[trigger_name].transitions[state.name]:
                try:
                    _ = self.get_state(transition.dest) if transition.dest is not None else transition.source
                except ValueError:
                    continue

                event_data.transition = transition
                try:
                    self.callbacks(self.prepare_event, event_data)
                    self.callbacks(transition.prepare, event_data)
                    if all(c.check(event_data) for c in transition.conditions):
                        return True
                except BaseException as err:
                    event_data.error = err
                    if self.on_exception:
                        self.callbacks(self.on_exception, event_data)
                    else:
                        raise
        return False

    def _add_may_transition_func_for_trigger(self, trigger, model):
        self._checked_assignment(model, "may_%s" % trigger, partial(self._can_trigger, model, trigger))

    def _add_trigger_to_model(self, trigger, model):
        self._checked_assignment(model, trigger, partial(self.events[trigger].trigger, model))
        self._add_may_transition_func_for_trigger(trigger, model)

    def _get_trigger(self, model, trigger_name, *args, **kwargs):
        """Convenience function added to the model to trigger events by name.
        Args:
            model (object): Model with assigned event trigger.
            trigger_name (str): Name of the trigger to be called.
            *args: Variable length argument list which is passed to the triggered event.
            **kwargs: Arbitrary keyword arguments which is passed to the triggered event.
        Returns:
            bool: True if a transitions has been conducted or the trigger event has been queued.
        """
        try:
            event = self.events[trigger_name]
        except KeyError:
            state = self.get_model_state(model)
            ignore = state.ignore_invalid_triggers if state.ignore_invalid_triggers is not None \
                else self.ignore_invalid_triggers
            if not ignore:
                raise AttributeError("Do not know event named '%s'." % trigger_name)
            return False
        return event.trigger(model, *args, **kwargs)

    def get_triggers(self, *args):
        """Collects all triggers FROM certain states.
        Args:
            *args: Tuple of source states.

        Returns:
            list of transition/trigger names.
        """
        names = {state.name if hasattr(state, 'name') else state for state in args}
        return [t for (t, ev) in self.events.items() if any(name in ev.transitions for name in names)]

    def add_transition(self, trigger, source, dest, conditions=None,
                       unless=None, before=None, after=None, prepare=None, **kwargs):
        """Create a new Transition instance and add it to the internal list.
        Args:
            trigger (str): The name of the method that will trigger the
                transition. This will be attached to the currently specified
                model (e.g., passing trigger='advance' will create a new
                advance() method in the model that triggers the transition.)
            source(str, Enum or list): The name of the source state--i.e., the state we
                are transitioning away from. This can be a single state, a
                list of states or an asterisk for all states.
            dest (str or Enum): The name of the destination State--i.e., the state
                we are transitioning into. This can be a single state or an
                equal sign to specify that the transition should be reflexive
                so that the destination will be the same as the source for
                every given source. If dest is None, this transition will be
                an internal transition (exit/enter callbacks won't be processed).
            conditions (str or list): Condition(s) that must pass in order
                for the transition to take place. Either a list providing the
                name of a callable, or a list of callables. For the transition
                to occur, ALL callables must return True.
            unless (str or list): Condition(s) that must return False in order
                for the transition to occur. Behaves just like conditions arg
                otherwise.
            before (str or list): Callables to call before the transition.
            after (str or list): Callables to call after the transition.
            prepare (str or list): Callables to call when the trigger is activated
            **kwargs: Additional arguments which can be passed to the created transition.
                This is useful if you plan to extend Machine.Transition and require more parameters.
        """
        if trigger == self.model_attribute:
            raise ValueError("Trigger name cannot be same as model attribute name.")
        if trigger not in self.events:
            self.events[trigger] = self._create_event(trigger, self)
            for model in self.models:
                self._add_trigger_to_model(trigger, model)

        if source == self.wildcard_all:
            source = list(self.states.keys())
        else:
            # states are checked lazily which means we will only raise exceptions when the passed state
            # is a State object because of potential confusion (see issue #155 for more details)
            source = [s.name if isinstance(s, State) and self._has_state(s, raise_error=True) or hasattr(s, 'name') else
                      s for s in listify(source)]

        for state in source:
            if dest == self.wildcard_same:
                _dest = state
            elif dest is not None:
                if isinstance(dest, State):
                    _ = self._has_state(dest, raise_error=True)
                _dest = dest.name if hasattr(dest, 'name') else dest
            else:
                _dest = None
            _trans = self._create_transition(state, _dest, conditions, unless, before,
                                             after, prepare, **kwargs)
            self.events[trigger].add_transition(_trans)

    def add_transitions(self, transitions):
        """Add several transitions.

        Args:
            transitions (list): A list of transitions.

        """
        for trans in listify(transitions):
            if isinstance(trans, list):
                self.add_transition(*trans)
            else:
                self.add_transition(**trans)

    def add_ordered_transitions(self, states=None, trigger='next_state',
                                loop=True, loop_includes_initial=True,
                                conditions=None, unless=None, before=None,
                                after=None, prepare=None, **kwargs):
        """Add a set of transitions that move linearly from state to state.
        Args:
            states (list): A list of state names defining the order of the
                transitions. E.g., ['A', 'B', 'C'] will generate transitions
                for A --> B, B --> C, and C --> A (if loop is True). If states
                is None, all states in the current instance will be used.
            trigger (str): The name of the trigger method that advances to
                the next state in the sequence.
            loop (boolean): Whether to add a transition from the last
                state to the first state.
            loop_includes_initial (boolean): If no initial state was defined in
                the machine, setting this to True will cause the _initial state
                placeholder to be included in the added transitions. This argument
                has no effect if the states argument is passed without the
                initial state included.
            conditions (str or list): Condition(s) that must pass in order
                for the transition to take place. Either a list providing the
                name of a callable, or a list of callables. For the transition
                to occur, ALL callables must return True.
            unless (str or list): Condition(s) that must return False in order
                for the transition to occur. Behaves just like conditions arg
                otherwise.
            before (str or list): Callables to call before the transition.
            after (str or list): Callables to call after the transition.
            prepare (str or list): Callables to call when the trigger is activated
            **kwargs: Additional arguments which can be passed to the created transition.
                This is useful if you plan to extend Machine.Transition and require more parameters.
        """
        if states is None:
            states = list(self.states.keys())  # need to listify for Python3
        len_transitions = len(states)
        if len_transitions < 2:
            raise ValueError("Can't create ordered transitions on a Machine "
                             "with fewer than 2 states.")
        if not loop:
            len_transitions -= 1
        # ensure all args are the proper length
        conditions = _prep_ordered_arg(len_transitions, conditions)
        unless = _prep_ordered_arg(len_transitions, unless)
        before = _prep_ordered_arg(len_transitions, before)
        after = _prep_ordered_arg(len_transitions, after)
        prepare = _prep_ordered_arg(len_transitions, prepare)
        # reorder list so that the initial state is actually the first one
        try:
            idx = states.index(self._initial)
            states = states[idx:] + states[:idx]
            first_in_loop = states[0 if loop_includes_initial else 1]
        except ValueError:
            # since initial is not part of states it shouldn't be part of the loop either
            first_in_loop = states[0]

        for i in range(0, len(states) - 1):
            self.add_transition(trigger, states[i], states[i + 1],
                                conditions=conditions[i],
                                unless=unless[i],
                                before=before[i],
                                after=after[i],
                                prepare=prepare[i],
                                **kwargs)
        if loop:
            self.add_transition(trigger, states[-1],
                                # omit initial if not loop_includes_initial
                                first_in_loop,
                                conditions=conditions[-1],
                                unless=unless[-1],
                                before=before[-1],
                                after=after[-1],
                                prepare=prepare[-1],
                                **kwargs)

    def get_transitions(self, trigger="", source="*", dest="*"):
        """Return the transitions from the Machine.
        Args:
            trigger (str): Trigger name of the transition.
            source (str, Enum or State): Limits list to transitions from a certain state.
            dest (str, Enum or State): Limits list to transitions to a certain state.
        """
        if trigger:
            try:
                events = (self.events[trigger], )
            except KeyError:
                return []
        else:
            events = self.events.values()
        transitions = []
        for event in events:
            transitions.extend(
                itertools.chain.from_iterable(event.transitions.values()))
        target_source = source.name if hasattr(source, 'name') else source if source != "*" else ""
        target_dest = dest.name if hasattr(dest, 'name') else dest if dest != "*" else ""
        return [transition
                for transition in transitions
                if (transition.source, transition.dest) == (target_source or transition.source,
                                                            target_dest or transition.dest)]

    def remove_transition(self, trigger, source="*", dest="*"):
        """Removes a transition from the Machine and all models.
        Args:
            trigger (str): Trigger name of the transition.
            source (str, Enum or State): Limits removal to transitions from a certain state.
            dest (str, Enum or State): Limits removal to transitions to a certain state.
        """
        source = listify(source) if source != "*" else source
        dest = listify(dest) if dest != "*" else dest
        # outer comprehension, keeps events if inner comprehension returns lists with length > 0
        tmp = {key: value for key, value in
               {k: [t for t in v
                    # keep entries if source should not be filtered; same for dest.
                    if (source != "*" and t.source not in source) or (dest != "*" and t.dest not in dest)]
                   # }.items() takes the result of the inner comprehension and uses it
                   # for the outer comprehension (see first line of comment)
                for k, v in self.events[trigger].transitions.items()}.items()
               if len(value) > 0}
        # convert dict back to defaultdict in case tmp is not empty
        if tmp:
            self.events[trigger].transitions = defaultdict(list, **tmp)
        # if no transition is left remove the trigger from the machine and all models
        else:
            for model in self.models:
                delattr(model, trigger)
            del self.events[trigger]

    def dispatch(self, trigger, *args, **kwargs):
        """Trigger an event on all models assigned to the machine.
        Args:
            trigger (str): Event name
            *args (list): List of arguments passed to the event trigger
            **kwargs (dict): Dictionary of keyword arguments passed to the event trigger
        Returns:
            bool The truth value of all triggers combined with AND
        """
        res = [getattr(model, trigger)(*args, **kwargs) for model in self.models]
        return all(res)

    def callbacks(self, funcs, event_data):
        """Triggers a list of callbacks"""
        for func in funcs:
            self.callback(func, event_data)
            _LOGGER.info("%sExecuted callback '%s'", self.name, func)

    def callback(self, func, event_data):
        """Trigger a callback function with passed event_data parameters. In case func is a string,
            the callable will be resolved from the passed model in event_data. This function is not intended to
            be called directly but through state and transition callback definitions.
        Args:
            func (str or callable): The callback function.
                1. First, if the func is callable, just call it
                2. Second, we try to import string assuming it is a path to a func
                3. Fallback to a model attribute
            event_data (EventData): An EventData instance to pass to the
                callback (if event sending is enabled) or to extract arguments
                from (if event sending is disabled).
        """

        func = self.resolve_callable(func, event_data)
        if self.send_event:
            func(event_data)
        else:
            func(*event_data.args, **event_data.kwargs)

    @staticmethod
    def resolve_callable(func, event_data):
        """Converts a model's property name, method name or a path to a callable into a callable.
            If func is not a string it will be returned unaltered.
        Args:
            func (str or callable): Property name, method name or a path to a callable
            event_data (EventData): Currently processed event
        Returns:
            callable function resolved from string or func
        """
        if isinstance(func, string_types):
            try:
                func = getattr(event_data.model, func)
                if not callable(func):  # if a property or some other not callable attribute was passed
                    def func_wrapper(*_, **__):  # properties cannot process parameters
                        return func
                    return func_wrapper
            except AttributeError:
                try:
                    module_name, func_name = func.rsplit('.', 1)
                    module = __import__(module_name)
                    for submodule_name in module_name.split('.')[1:]:
                        module = getattr(module, submodule_name)
                    func = getattr(module, func_name)
                except (ImportError, AttributeError, ValueError):
                    raise AttributeError("Callable with name '%s' could neither be retrieved from the passed "
                                         "model nor imported from a module." % func)
        return func

    def _has_state(self, state, raise_error=False):
        found = state in self.states.values()
        if not found and raise_error:
            msg = 'State %s has not been added to the machine' % (state.name if hasattr(state, 'name') else state)
            raise ValueError(msg)
        return found

    def _process(self, trigger):

        # default processing
        if not self.has_queue:
            if not self._transition_queue:
                # if trigger raises an Error, it has to be handled by the Machine.process caller
                return trigger()
            raise MachineError("Attempt to process events synchronously while transition queue is not empty!")

        # process queued events
        self._transition_queue.append(trigger)
        # another entry in the queue implies a running transition; skip immediate execution
        if len(self._transition_queue) > 1:
            return True

        # execute as long as transition queue is not empty
        while self._transition_queue:
            try:
                self._transition_queue[0]()
                self._transition_queue.popleft()
            except BaseException:
                # if a transition raises an exception, clear queue and delegate exception handling
                self._transition_queue.clear()
                raise
        return True

    def _identify_callback(self, name):
        # Does the prefix match a known callback?
        for callback in itertools.chain(self.state_cls.dynamic_methods, self.transition_cls.dynamic_methods):
            if name.startswith(callback):
                callback_type = callback
                break
        else:
            return None, None

        # Extract the target by cutting the string after the type and separator
        target = name[len(callback_type) + len(self.separator):]

        # Make sure there is actually a target to avoid index error and enforce _ as a separator
        if target == '' or name[len(callback_type)] != self.separator:
            return None, None

        return callback_type, target

    def __getattr__(self, name):
        # Machine.__dict__ does not contain double underscore variables.
        # Class variables will be mangled.
        if name.startswith('__'):
            raise AttributeError("'{}' does not exist on <Machine@{}>"
                                 .format(name, id(self)))

        # Could be a callback
        callback_type, target = self._identify_callback(name)

        if callback_type is not None:
            if callback_type in self.transition_cls.dynamic_methods:
                if target not in self.events:
                    raise AttributeError("event '{}' is not registered on <Machine@{}>"
                                         .format(target, id(self)))
                return partial(self.events[target].add_callback, callback_type)

            if callback_type in self.state_cls.dynamic_methods:
                state = self.get_state(target)
                return partial(state.add_callback, callback_type[3:])

        try:
            return self.__getattribute__(name)
        except AttributeError:
            # Nothing matched
            raise AttributeError("'{}' does not exist on <Machine@{}>".format(name, id(self)))


class MachineError(Exception):
    """MachineError is used for issues related to state transitions and current states.
    For instance, it is raised for invalid transitions or machine configuration issues.
    """

    def __init__(self, value):
        super(MachineError, self).__init__(value)
        self.value = value

    def __str__(self):
        return repr(self.value)


# ==== vendored transitions.version ====
# Contains the current version of transition which is used in setup.py and can also be used
#     to determine transitions' version during runtime.

__version__ = '0.9.3'


# ==== common/errors.py ====
# atm's errors (docs/layered-architecture.md §4.2, §5): atm-lib raises them by what went wrong, and each front end
# says it its own way: the CLI as a message and an exit code (`code`), the REST API as an HTTP status.

class AtmFailure(Exception):
    """Something atm can't do, with a message for the person or agent who asked. `code` is the CLI's exit code."""

    code = 1

    def __init__(self, message, code=None):
        super().__init__(message)
        if code is not None:
            self.code = code


class NotFound(AtmFailure):
    """No such task, sprint, release, component or project."""


class Invalid(AtmFailure):
    """The request itself is wrong: an unknown option value, a malformed ID (exit code 2, like a usage error)."""

    code = 2


class Refused(AtmFailure):
    """The thing exists but atm won't hand it out: a project file it doesn't show (hidden, ignored, a key file)."""


class TooLarge(Refused):
    """Larger than atm hands out."""


class Conflict(AtmFailure):
    """The data changed since it was read, or the change isn't possible in the task's state."""


class NothingAvailable(AtmFailure):
    """No task to hand out (exit code 3, `atm next`)."""

    code = 3


class Claimed(AtmFailure):
    """Another agent holds the task, or the caller's lease ran out (exit code 4)."""

    code = 4


# ==== common/vocabularies.py ====
# The status kinds and the default vocabularies, generated from common/schema/vocabularies.json by
# atm-cli/build.py: don't edit it, change that file and run `pnpm run build:cli` in dashboard/ (the pre-commit hook does it too).

KINDS = ('todo', 'active', 'review', 'blocked', 'done', 'skipped')
DEFAULT_STATUSES = [
    {
        "id": "todo",
        "label": "To Do",
        "kind": "todo"
    },
    {
        "id": "in_progress",
        "label": "In Progress",
        "kind": "active"
    },
    {
        "id": "in_review",
        "label": "In Review",
        "kind": "review"
    },
    {
        "id": "blocked",
        "label": "Blocked",
        "kind": "blocked"
    },
    {
        "id": "done",
        "label": "Done",
        "kind": "done"
    },
    {
        "id": "skipped",
        "label": "Skipped",
        "kind": "skipped"
    }
]
DEFAULT_TYPES = [
    {
        "id": "feature",
        "label": "Feature"
    },
    {
        "id": "bugfix",
        "label": "Bugfix"
    },
    {
        "id": "test",
        "label": "Test"
    },
    {
        "id": "infra",
        "label": "Infra"
    },
    {
        "id": "docs",
        "label": "Docs"
    },
    {
        "id": "refactor",
        "label": "Refactor"
    },
    {
        "id": "polish",
        "label": "Polish"
    }
]
DEFAULT_PRIORITIES = [
    {
        "id": "P0",
        "label": "Critical"
    },
    {
        "id": "P1",
        "label": "High"
    },
    {
        "id": "P2",
        "label": "Medium"
    },
    {
        "id": "P3",
        "label": "Low"
    }
]
DEFAULT_PRIORITY = "P2"


# ==== common/core.py ====
# Constants, errors, output, time helpers and atomic writes, shared by every layer. Running git is not here: it is
# the host layer's (host/git.py), for the store and the vcs only.

# Where the built plugin/scripts/atm.py lives (only the built file is run, never these modules)
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
# Built, this is plugin/scripts/atm.py and the plugin is its parent folder. Run from source (atm2, atm-lib's unit
# tests), this is atm-cli/atm/common/core.py, and the plugin is plugin/ in the same checkout.
_SOURCE = os.path.basename(SCRIPT_DIR) == "common" and os.path.basename(os.path.dirname(SCRIPT_DIR)) == "atm"
PLUGIN_ROOT = (
    os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(SCRIPT_DIR))), "plugin")
    if _SOURCE
    else os.path.dirname(SCRIPT_DIR)
)
TASK_DIR = ".agentic-task-manager"
TASKS_FILE = "tasks.json"
DASHBOARD_FILE = "tasks-dashboard.html"


class AtmError(AtmFailure):
    """The errors of the modules from before the layers: atm's errors (common/errors.py) without a kind."""

    def __init__(self, message, code=1):
        super().__init__(message, code)


class AtmExit(Exception):
    """End with an exit code after normal output (not an error)."""

    def __init__(self, code):
        super().__init__(code)
        self.code = code


def out(text=""):
    sys.stdout.write(text + "\n")


def to_json(value):
    return json.dumps(value, indent=2, ensure_ascii=False)


def rel(root, p):
    return os.path.relpath(p, root)


def atm_home():
    """The user's atm folder: ATM_HOME, else ~/.agentic-task-manager (the registry and the server's record)."""
    return os.path.abspath(os.path.expanduser(os.environ.get("ATM_HOME") or "~/.agentic-task-manager"))


def ensure_home(home):
    """Create the atm folder readable by its owner only: it lists the user's projects and the server's pid and port
    (docs/security-review.md F18). A folder that exists already keeps its permissions."""
    os.makedirs(home, mode=0o700, exist_ok=True)


def plugin_version():
    with open(os.path.join(PLUGIN_ROOT, ".claude-plugin", "plugin.json"), encoding="utf-8") as f:
        return json.load(f)["version"]


def parse_version(text):
    parts = []
    for part in (text or "").split("."):
        m = re.match(r"\d+", part)
        parts.append(int(m.group()) if m else 0)
    return parts


def now():
    """The current time; ATM_NOW (ISO 8601 UTC) overrides it, for tests."""
    fixed = os.environ.get("ATM_NOW")
    return parse_iso(fixed) if fixed else datetime.now(timezone.utc)


def now_iso():
    return now().strftime("%Y-%m-%dT%H:%M:%SZ")


def parse_iso(text):
    return datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)


def format_duration(seconds):
    seconds = max(0, int(seconds))
    if seconds < 60:
        return f"{seconds}s"
    if seconds < 3600:
        return f"{seconds // 60}m"
    return f"{seconds // 3600}h {seconds % 3600 // 60}m"


def fsync_dir(directory):
    """Make a rename in `directory` durable. Not every platform can open a directory; that's fine to skip."""
    try:
        fd = os.open(directory, os.O_RDONLY)
    except OSError:
        return
    try:
        os.fsync(fd)
    except OSError:
        pass
    finally:
        os.close(fd)


def write_text_atomic(p, text):
    """Replace a file so readers only ever see the complete old or the complete new content."""
    tmp = f"{p}.{os.getpid()}.{os.urandom(4).hex()}.tmp"
    try:
        with open(tmp, "w", encoding="utf-8") as f:
            f.write(text)
            f.flush()
            os.fsync(f.fileno())
        os.replace(tmp, p)
    except BaseException:
        with contextlib.suppress(OSError):
            os.remove(tmp)
        raise
    fsync_dir(os.path.dirname(os.path.abspath(p)))


def write_json_atomic(p, value):
    write_text_atomic(p, to_json(value) + "\n")


def read_json(p):
    """The file's JSON, or None when it can't be read or parsed. Deeply nested JSON (a hostile file) raises
    RecursionError in the parser; that is unreadable too, not a crash (docs/security-review.md F12, F18)."""
    try:
        with open(p, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError, RecursionError):
        return None


def is_zombie(pid):
    """A process that has exited but wasn't reaped yet (common in containers without an init process). Linux only;
    elsewhere an orphaned process is reaped by the system right away."""
    try:
        with open(f"/proc/{pid}/stat", encoding="utf-8") as f:
            stat = f.read()
    except OSError:
        return False
    fields = stat[stat.rfind(")") + 1 :].split()
    return bool(fields) and fields[0] == "Z"


def pid_alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return not is_zombie(pid)


WHOAMI = "/__atm/whoami"


def whoami(port, timeout=1):
    """Ask the atm server on this port who it is; None if nothing (or not an atm server) answers."""
    import urllib.request

    try:
        with urllib.request.urlopen(f"http://localhost:{port}{WHOAMI}", timeout=timeout) as response:
            info = json.loads(response.read().decode("utf-8"))
    except (OSError, ValueError):
        return None
    return info if isinstance(info, dict) and info.get("atm") else None


# ==== common/dashboard.py ====
# What the CLI and the REST API's server both know about the dashboard server (docs/layered-architecture.md §11.1):
# the plugin's dashboard file (plugin/templates/tasks-dashboard.html) and the version and build tags at its top, the
# server's record and log in the user's atm folder, and the command that starts the server process. The CLI starts,
# stops and refreshes the server as a process; the server itself is the REST API's (`api/server.py`).

TEMPLATE = os.path.join(PLUGIN_ROOT, "templates", DASHBOARD_FILE)
VERSION_META = re.compile(r'<meta name="atm-dashboard-version" content="([^"]*)"')
BUILD_META = re.compile(r'<meta name="atm-dashboard-build" content="([^"]*)"')
# The tags sit at the top of the page; reading this much finds them without reading the whole (inlined) dashboard
HEAD_BYTES = 16384

# In the user's atm folder (ATM_HOME): the running server's {pid, port, version, build, plugin_root, home, started_at}
SERVER_RECORD = "server.json"
SERVER_LOG = "server.log"
DEFAULT_PORT = 8765
# How many ports from the first one `atm serve status` and `stop` look at for a server the record doesn't name (T-390)
SCAN_PORTS = 20
# The argument the built atm.py takes to run the server instead of the CLI (plugin/scripts/atm-serve passes it)
SERVER_ENTRY = "--server"


def home_id(home):
    """A short ID of an atm folder: its real path, hashed. The server gives it in whoami and its record, so the CLI can
    tell its own folder's servers from another folder's (T-390) without the path going over HTTP."""
    return hashlib.sha256(os.path.realpath(home).encode("utf-8")).hexdigest()[:16]


def page_tags(head):
    """(version, build ID) from the start of a dashboard page (text), "" for a missing tag."""
    version, build = VERSION_META.search(head), BUILD_META.search(head)
    return (version.group(1) if version else "", build.group(1) if build else "")


def dashboard_head(p):
    """(version, build ID) of a dashboard file, "" for a missing tag; None if there is no file."""
    try:
        with open(p, encoding="utf-8", errors="replace") as f:
            head = f.read(HEAD_BYTES)
    except OSError:
        return None
    return page_tags(head)


def server_command(home, port):
    """The command that runs the server process for the atm folder `home`, from the first free port from `port`: this
    plugin's atm.py, with this Python, entered as the server (what plugin/scripts/atm-serve runs)."""
    return [sys.executable, os.path.join(SCRIPT_DIR, "atm.py"), SERVER_ENTRY, home, str(port)]


# ==== common/model.py ====
# The rules of task data: the project's vocabularies, looking tasks up, and the status transitions themselves.
# Reading and writing the files is the store's (db/jsongit/files.py).
#
# Every rule is written against a status's kind, never its name (docs/v4/data-model.md §3): a project may name its own
# statuses in meta.statuses, each with one of the six kinds below.

# The kinds and the default vocabularies are common's (common/schema/vocabularies.json), shared with the dashboard
WORKFLOW_TYPES = ("direct", "pull-request")

# command: (kinds a task may be in, kind it moves to)
STATUS_CHANGES = {
    # From review only once the review asked for rework or approved it (SR-3.9)
    "start": (("todo", "review"), "active"),
    "review": (("active",), "review"),
    "done": (("active", "review"), "done"),
    "fail": (("active",), "todo"),
    "release": (("active",), "todo"),
    "reject": (("review",), "todo"),
    # The review's outcome; the task stays in review until an agent takes it back (SR-6.1)
    "approve": (("review",), "review"),
    "request-changes": (("review",), "review"),
    "block": (("todo", "active"), "blocked"),
    "unblock": (("blocked",), "todo"),
    # Undo a done (or skipped) by mistake, or work that turns out unfinished; the reason is kept as a note
    "reopen": (("done", "skipped"), "todo"),
    # A task that turns out not to be needed; reopen undoes it
    "skip": (("todo", "blocked"), "skipped"),
}
WAITING_ON = ("user", "agent")
# Review states that make a task in review work for an agent again (SR-3.9)
REWORK_STATES = ("changes_requested", "approved")
# How a finished task ended (SR-2.8): stored only when it isn't the default, `done` for a done task and `wont_do` for
# a skipped one
RESOLUTIONS = ("done", "wont_do", "duplicate", "obsolete")
SKIP_RESOLUTIONS = ("wont_do", "obsolete", "duplicate")


def _terms(meta, key, defaults, need_kind=False):
    """meta[key] as a list of {id, label[, kind]}, or the defaults when absent. Raises AtmError naming the problem."""
    if key not in meta:
        return defaults
    value = meta[key]
    if not isinstance(value, list) or not value:
        raise AtmError(f"meta.{key} must be a non-empty list")
    seen, terms = set(), []
    for item in value:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str) or not item["id"]:
            raise AtmError(f"meta.{key}: every entry needs an \"id\" (a non-empty string)")
        if item["id"] in seen:
            raise AtmError(f"meta.{key}: \"{item['id']}\" is listed twice")
        seen.add(item["id"])
        term = {"id": item["id"], "label": item.get("label") or item["id"].replace("_", " ").capitalize()}
        if need_kind:
            if item.get("kind") not in KINDS:
                raise AtmError(
                    f"meta.{key}: \"{item['id']}\" has the kind {item.get('kind')!r}; use one of {', '.join(KINDS)}"
                )
            term["kind"] = item["kind"]
        terms.append(term)
    return terms


WORKFLOW_DEFAULTS = {"verify": True, "max_rounds": 3, "commit_template": "feat({id}): {name}"}

# Strings from tasks.json that agents put into shell commands (a base branch, task IDs) are kept to characters a
# shell doesn't treat specially (docs/security-review.md F5): no `;`, `$`, quotes, spaces, or a leading `-`
BRANCH_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._/-]{0,199}")
TASK_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}")


def valid_branch(value):
    return isinstance(value, str) and bool(BRANCH_NAME.fullmatch(value)) and ".." not in value


def valid_task_id(value):
    return isinstance(value, str) and bool(TASK_ID.fullmatch(value))


def split_id(task_id):
    """(prefix, digits) of an ID that ends in a number ("T-001" -> ("T-", "001")), else None. A plain scan from the
    end, not a regex: `^(.*?)(\\d+)$` backtracks quadratically on a long run of digits without an end (a hostile ID,
    docs/security-review.md F12)."""
    if not isinstance(task_id, str):
        return None
    end = len(task_id)
    while end and task_id[end - 1].isdigit():
        end -= 1
    return (task_id[:end], task_id[end:]) if end < len(task_id) else None


def check_workflow(workflow):
    """meta.workflow (docs/v4/skills-and-workflows.md S-1.1), when set: raises AtmError naming what's wrong."""
    if workflow is None:
        return
    if not isinstance(workflow, dict) or workflow.get("type") not in WORKFLOW_TYPES:
        raise AtmError(f"meta.workflow.type must be one of {', '.join(WORKFLOW_TYPES)}")
    checks = (
        ("base", valid_branch, "a branch name (letters, digits, `.`, `_`, `/`, `-`; not starting with `-`)"),
        ("verify", lambda v: isinstance(v, bool), "true or false"),
        ("max_rounds", lambda v: isinstance(v, int) and not isinstance(v, bool) and v > 0, "a positive whole number"),
        ("commit_template", lambda v: isinstance(v, str) and v, "a message template like \"feat({id}): {name}\""),
        (
            "trailers",
            lambda v: v == "forbid"
            or (isinstance(v, dict) and isinstance(v.get("require"), list) and all(isinstance(x, str) for x in v["require"])),
            '"forbid" or {"require": ["Co-Authored-By: ..."]}',
        ),
    )
    for key, valid, expected in checks:
        if key in workflow and not valid(workflow[key]):
            raise AtmError(f"meta.workflow.{key} must be {expected}")


SPRINT_STATES = ("planned", "active", "closed")
SPRINT_NAMING = ("people", "time")
REVIEW_MODES = ("blocker", "async", "batch", "none")
MAX_SPRINT_NAME = 48
DATE = re.compile(r"\d{4}-\d{2}-\d{2}")


def check_sprints(meta):
    """meta.sprints (docs/sprints-and-releases-design.md SR-2.1, SR-2.3) as a list of sprints, in order; [] when
    absent. A sprint's ID is its name: unique (compared case-insensitively), at most 48 characters, no space at either
    end. A sprint without a state is planned. Raises AtmError naming the problem."""
    sprints = meta.get("sprints", [])
    if not isinstance(sprints, list):
        raise AtmError("meta.sprints must be a list of sprints")
    seen, active = set(), []
    for sprint in sprints:
        if not isinstance(sprint, dict) or not isinstance(sprint.get("id"), str) or not sprint["id"].strip():
            raise AtmError('meta.sprints: every sprint needs an "id", its name (a non-empty string)')
        name = sprint["id"]
        if name != name.strip() or len(name) > MAX_SPRINT_NAME:
            raise AtmError(f"meta.sprints: {name!r} is not a sprint name (at most {MAX_SPRINT_NAME} characters, no "
                           "space at either end)")
        if name.casefold() in seen:
            raise AtmError(f"meta.sprints: {name!r} is listed twice")
        seen.add(name.casefold())
        state = sprint.get("state", "planned")
        if state not in SPRINT_STATES:
            raise AtmError(f"meta.sprints: {name!r} has the state {state!r}; use one of {', '.join(SPRINT_STATES)}")
        if state == "active":
            active.append(name)
        if "due" in sprint and not (isinstance(sprint["due"], str) and DATE.fullmatch(sprint["due"])):
            raise AtmError(f"meta.sprints: {name!r} has the due date {sprint['due']!r}; use YYYY-MM-DD")
        for key in ("goal", "release", "start", "end"):
            if key in sprint and not isinstance(sprint[key], str):
                raise AtmError(f"meta.sprints: {name!r}'s {key} must be a string")
    if len(active) > 1:
        raise AtmError(f"meta.sprints: only one sprint can be active, but {' and '.join(active)} are")
    return [{**s, "state": s.get("state", "planned")} for s in sprints]


RELEASE_STATES = ("planned", "released", "archived")
# A release ID is any string of these (Q5): agents put it in shell commands, so no spaces or quotes
RELEASE_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}")


def check_releases(meta):
    """meta.releases (docs/sprints-and-releases-design.md SR-2.2) as a list, in order; [] when absent. A release
    without a state is planned. Raises AtmError naming the problem."""
    releases = meta.get("releases", [])
    if not isinstance(releases, list):
        raise AtmError("meta.releases must be a list of releases")
    seen = set()
    for release in releases:
        if not isinstance(release, dict) or not isinstance(release.get("id"), str) or not RELEASE_ID.fullmatch(
            release["id"]
        ):
            raise AtmError(
                'meta.releases: every release needs an "id" of letters, digits, ".", "-" and "_" (at most 64)'
            )
        if release["id"] in seen:
            raise AtmError(f"meta.releases: {release['id']!r} is listed twice")
        seen.add(release["id"])
        state = release.get("state", "planned")
        if state not in RELEASE_STATES:
            raise AtmError(
                f"meta.releases: {release['id']!r} has the state {state!r}; use one of {', '.join(RELEASE_STATES)}"
            )
        if "target" in release and not (isinstance(release["target"], str) and DATE.fullmatch(release["target"])):
            raise AtmError(f"meta.releases: {release['id']!r} has the target {release['target']!r}; use YYYY-MM-DD")
        for key in ("name", "released_at"):
            if key in release and not isinstance(release[key], str):
                raise AtmError(f"meta.releases: {release['id']!r}'s {key} must be a string")
    return [{**r, "state": r.get("state", "planned")} for r in releases]


def check_components(meta):
    """meta.components (docs/sprints-and-releases-design.md §2, SR-2.7) as a list, in order; [] when absent: each
    {id, label, paths (globs matched against a task's files), review}. Raises AtmError naming the problem."""
    components = meta.get("components", [])
    if not isinstance(components, list):
        raise AtmError("meta.components must be a list of components")
    seen = set()
    for component in components:
        if not isinstance(component, dict) or not isinstance(component.get("id"), str) or not RELEASE_ID.fullmatch(
            component["id"]
        ):
            raise AtmError(
                'meta.components: every component needs an "id" of letters, digits, ".", "-" and "_" (at most 64)'
            )
        name = component["id"]
        if name in seen:
            raise AtmError(f"meta.components: {name!r} is listed twice")
        seen.add(name)
        paths = component.get("paths", [])
        if not isinstance(paths, list) or not all(isinstance(p, str) and p for p in paths):
            raise AtmError(f"meta.components: {name!r}'s paths must be a list of globs (strings)")
        if "review" in component and component["review"] not in REVIEW_MODES:
            raise AtmError(f"meta.components: {name!r} has the review {component['review']!r}; use one of "
                           f"{', '.join(REVIEW_MODES)}")
        if "label" in component and not isinstance(component["label"], str):
            raise AtmError(f"meta.components: {name!r}'s label must be a string")
    return [{**c, "paths": c.get("paths", [])} for c in components]


def find_release(vocab, release_id):
    """The release with this ID, or None."""
    return next((r for r in vocab["releases"] if r["id"] == release_id), None)


def known_release(vocab, release_id):
    """`release_id`, when meta.releases has it; else an AtmError naming the releases."""
    if find_release(vocab, release_id) is None:
        known = ", ".join(r["id"] for r in vocab["releases"]) or "none yet: atm release add <R>"
        raise AtmError(f"no release {release_id!r}; the releases: {known}", 2)
    return release_id


def check_setting(meta, key, choices, default):
    value = meta.get(key, default)
    if value not in choices:
        raise AtmError(f"meta.{key} must be one of {', '.join(choices)}")
    return value


# The lists in meta (each its own entity in the store's interface, docs/layered-architecture.md §3.1): what isn't one of
# them is a setting (project, description, id, repository, logo, workflow, review, archive, …)
LIST_KEYS = ("statuses", "types", "priorities", "sprints", "releases", "components", "labels", "phases")


def settings_of(meta):
    """The project's settings: meta without its lists (LIST_KEYS)."""
    meta = meta if isinstance(meta, dict) else {}
    return {k: v for k, v in meta.items() if k not in LIST_KEYS}


def workflow_of(data):
    """The project's workflow with its defaults filled in, or None when it has none."""
    meta = data.get("meta") if isinstance(data.get("meta"), dict) else {}
    workflow = meta.get("workflow")
    if workflow is None:
        return None
    check_workflow(workflow)
    return {**WORKFLOW_DEFAULTS, **workflow}


def vocabulary(data):
    """The project's vocabularies from meta, with the defaults for what it doesn't define (DM-2). Raises AtmError,
    naming the problem, when meta defines something that can't work."""
    meta = data.get("meta") if isinstance(data.get("meta"), dict) else {}
    statuses = _terms(meta, "statuses", DEFAULT_STATUSES, need_kind=True)
    for kind in ("todo", "active", "done"):
        if not any(s["kind"] == kind for s in statuses):
            raise AtmError(f"meta.statuses needs at least one status of the kind {kind!r}")
    priorities = _terms(meta, "priorities", DEFAULT_PRIORITIES)
    ids = [p["id"] for p in priorities]
    # Without a default: P2 if the project has it, else the middle priority (as the dashboard does)
    default_priority = meta.get(
        "default_priority", DEFAULT_PRIORITY if DEFAULT_PRIORITY in ids else ids[(len(ids) - 1) // 2]
    )
    if default_priority not in ids:
        raise AtmError(f"meta.default_priority {default_priority!r} is not one of meta.priorities")
    # meta.phases, from before sprints, isn't read: atm migrate turns it into sprints (SR-8.1, SR-8.3)
    check_workflow(meta.get("workflow"))
    return {
        "statuses": statuses,
        "types": _terms(meta, "types", DEFAULT_TYPES),
        "priorities": priorities,
        "default_priority": default_priority,
        "sprints": check_sprints(meta),
        "releases": check_releases(meta),
        "sprint_names": check_setting(meta, "sprint_names", SPRINT_NAMING, "people"),
        "review": check_setting(meta, "review", REVIEW_MODES, "none"),
        "components": check_components(meta),
    }


def find_sprint(vocab, name):
    """The sprint with this name (compared case-insensitively), or None."""
    if not isinstance(name, str):
        return None
    for sprint in vocab["sprints"]:
        if sprint["id"].casefold() == name.casefold():
            return sprint
    return None


def set_sprint(task, sprint, stamp):
    """Put `task` in `sprint` (a sprint ID), or in the backlog (None). Every change ends the open sprint_history entry
    and, for a sprint, starts a new one (SR-2.5). Returns whether anything changed."""
    if task.get("sprint") == sprint:
        return False
    history = task.setdefault("sprint_history", [])
    for entry in history:
        if isinstance(entry, dict) and "to" not in entry:
            entry["to"] = stamp
    if sprint is None:
        task.pop("sprint", None)
    else:
        task["sprint"] = sprint
        history.append({"sprint": sprint, "from": stamp})
    if not history:
        task.pop("sprint_history")
    return True


def effective_release(vocab, task):
    """A task's release: its own, else its sprint's (SR-2.6), or None."""
    if task.get("release"):
        return task["release"]
    sprint = find_sprint(vocab, task.get("sprint"))
    return sprint.get("release") if sprint else None


def effective_review(vocab, task):
    """A task's review mode: its own, else its first component's that has one, else the project's (meta.review),
    else none (SR-2.6)."""
    if task.get("review") in REVIEW_MODES:
        return task["review"]
    components = {c.get("id"): c for c in vocab["components"] if isinstance(c, dict)}
    for component in task.get("components") or []:
        mode = components.get(component, {}).get("review")
        if mode in REVIEW_MODES:
            return mode
    return vocab["review"]


def kind_of(vocab, status):
    """The status's kind. A status the project doesn't list keeps its default kind if it is a default status; any
    other status has none (None)."""
    for s in vocab["statuses"]:
        if s["id"] == status:
            return s["kind"]
    for s in DEFAULT_STATUSES:
        if s["id"] == status:
            return s["kind"]
    return None


def open_in_closed_sprints(vocab, tasks):
    """Unfinished tasks whose sprint is closed (T-330): reopened there, or left by a merge. The loop's default selection
    (the active sprint) never takes them. [{id, name, status, sprint}]"""
    found = []
    for task in tasks:
        sprint = find_sprint(vocab, task.get("sprint")) if isinstance(task, dict) else None
        if sprint and sprint["state"] == "closed" and kind_of(vocab, task.get("status")) not in ("done", "skipped"):
            found.append({"id": task.get("id"), "name": task.get("name", ""), "status": task.get("status"), "sprint": sprint["id"]})
    return found


def move_out_hint(task_id, sprint):
    """The end of a sentence on a task open in a closed sprint (T-330), with what to do: a suggestion, atm moves
    nothing on its own."""
    return f"the closed sprint {sprint!r}, where the loop won't take it: atm sprint move {task_id} --to backlog (or --to active)"


def priority_rank(vocab, priority):
    """0 for the most urgent priority. A missing or unknown priority ranks as the default priority."""
    ids = [p["id"] for p in vocab["priorities"]]
    return ids.index(priority) if priority in ids else ids.index(vocab["default_priority"])


def statuses_of(vocab, kind):
    return [s["id"] for s in vocab["statuses"] if s["kind"] == kind]


def status_for(vocab, kind, chosen=None):
    """The status a task moving to `kind` gets: `chosen` (--status), which must be of that kind, else the first status
    of that kind the project lists."""
    options = statuses_of(vocab, kind)
    if chosen is None:
        return options[0]
    if chosen not in options:
        known = kind_of(vocab, chosen)
        what = f"its kind is {known}" if known else "the project has no such status"
        raise AtmError(f"--status {chosen} is not one of the {kind} statuses ({what}); use {' or '.join(options)}", 2)
    return chosen


# The rules that need an archived task (a dependency only in the archive is done, the "it is archived" hint) take
# `archived`: a lookup, `archived(task_id)` -> the archived task or None, from the project's store
# (`store.get_archived_task`, or atm-lib's `archived_in(work)` inside a change). There is no global one: one process
# (atm serve) works on many projects at once (docs/layered-architecture.md §11.4).

ARCHIVE_DEFAULTS = {"threshold_kb": 1024, "threshold_tasks": 500, "keep_days": 14, "auto": True}


def archive_settings(data):
    """meta.archive, with the defaults for what it doesn't set (DM-2)."""
    meta = data.get("meta") if isinstance(data.get("meta"), dict) else {}
    given = meta.get("archive") if isinstance(meta.get("archive"), dict) else {}
    settings = dict(ARCHIVE_DEFAULTS)
    for key, default in ARCHIVE_DEFAULTS.items():
        value = given.get(key)
        if isinstance(default, bool):
            if isinstance(value, bool):
                settings[key] = value
        elif isinstance(value, (int, float)) and not isinstance(value, bool) and value >= 0:
            settings[key] = value
    return settings


def finished_at(task):
    """When a finished task finished: completed_at (done) or skipped_at (skipped), else updated_at for files from
    before those were written (atm migrate fills them in)."""
    for key in ("completed_at", "skipped_at", "updated_at"):
        try:
            return parse_iso(task[key])
        except (KeyError, TypeError, ValueError):
            continue
    return None


def archivable(data, before=None, keep=None):
    """The tasks `atm archive` would move (DM-5.1): done- and skipped-kind, not finished in the last `keep` days
    (default meta.archive.keep_days), not in an open PR; only those finished before `before`, if given."""
    vocab = vocabulary(data)
    recent = now() - timedelta(days=archive_settings(data)["keep_days"] if keep is None else keep)
    result = []
    for t in data["tasks"]:
        if not isinstance(t, dict) or kind_of(vocab, t.get("status")) not in ("done", "skipped"):
            continue
        if isinstance(t.get("pr"), dict) and not t.get("merged_at"):
            continue  # still in an open PR
        finished = finished_at(t)
        if finished is not None and finished > recent:
            continue
        if before is not None and (finished is None or finished >= before):
            continue
        result.append(t)
    return result


def format_size(size):
    return f"{size / 1048576:.1f} MB" if size >= 1048576 else f"{size // 1024} KB"


def archive_hint_for(data, size):
    """The suggestion to archive (CLI-5.1), or None: when the task data (`size` bytes) is over
    meta.archive.threshold_kb, or has more archivable tasks than threshold_tasks, and there is something to move."""
    settings = archive_settings(data)
    movable = len(archivable(data))
    if not movable or (size <= settings["threshold_kb"] * 1024 and movable <= settings["threshold_tasks"]):
        return None
    vocab = vocabulary(data)
    finished = sum(
        1 for t in data["tasks"] if isinstance(t, dict) and kind_of(vocab, t.get("status")) in ("done", "skipped")
    )
    return {
        "bytes": size,
        "finished": finished,
        "archivable": movable,
        "auto": settings["auto"],  # false: the skills only relay the hint (S-3.5)
        "text": f"tasks.json is {format_size(size)} with {finished} finished tasks; `atm db archive` would move "
        f"{movable} of them to tasks-archive.json.",
    }


META_KEY = re.compile(r"meta\.(\w+)")


def tolerant_vocabulary(meta):
    """The vocabularies of `meta` as far as they work, and what doesn't: each list or setting `vocabulary` refuses is
    left out (its default used instead), as the dashboard reads them. Returns (vocabulary, problems, the meta without
    what was left out)."""
    usable = dict(meta) if isinstance(meta, dict) else {}
    problems = []
    while True:
        try:
            return vocabulary({"meta": usable}), problems, usable
        except AtmError as e:
            problems.append(str(e))
            key = META_KEY.match(str(e))
            if key and key.group(1) in usable:
                usable.pop(key.group(1))
                if key.group(1) == "priorities":
                    usable.pop("default_priority", None)  # it names one of them
            else:
                usable = {}  # nothing to narrow it to: the defaults


def find_task(data, task_id, archived=None):
    """The task with this ID in tasks.json. Raises AtmError when there is none; with `archived` (the project's lookup)
    the error says when the task is archived."""
    for task in data["tasks"]:
        if task.get("id") == task_id:
            return task
    hint = " (it is archived: atm db unarchive brings it back)" if archived and archived(task_id) else ""
    raise AtmError(f"no task {task_id}{hint}")


def unfinished_dependencies(data, task, archived):
    """Dependencies that don't have a done-kind status yet, with their status ("missing" for unknown IDs). A
    dependency that is only in the archive is done (DM-3.1): `archived` is the project's lookup."""
    vocab = vocabulary(data)
    status = {t.get("id"): t.get("status") for t in data["tasks"]}
    deps = task.get("dependencies", [])
    # A list of IDs, as load_tasks checks; a file read tolerantly (load_tasks_tolerant) may hold anything there
    deps = [d for d in deps if isinstance(d, str)] if isinstance(deps, list) else []
    return [
        (dep, status.get(dep, "missing"))
        for dep in deps
        if (dep in status and kind_of(vocab, status[dep]) != "done") or (dep not in status and not archived(dep))
    ]


def natural_key(task_id):
    """Sort T-2 before T-10, like the dashboard's localeCompare(..., { numeric: true })."""
    return [int(part) if part.isdigit() else part.lower() for part in re.split(r"(\d+)", str(task_id))]


MAX_NOTE = 500


def check_note(text):
    """Notes stay short; anything longer belongs in a file in the repository."""
    if len(text) > MAX_NOTE:
        raise AtmError(
            f"a note is at most {MAX_NOTE} characters (this one has {len(text)}); "
            "put longer text in a file in the repository and mention it in the note",
            2,
        )
    return text


def add_note(task, kind, text, by, stamp, checked=True):
    """Append a note (DM-4): handoff, outcome, review, rejection, failure, answer or note. `checked=False` for a note
    built from parts already checked one by one (an unblock's questions and answers)."""
    note = {"at": stamp, "kind": kind, "text": check_note(text) if checked else text}
    if by:
        note["by"] = by
    task.setdefault("notes", []).append(note)


def touch(task, actor, stamp):
    """Every change records when and by whom (CLI-3.5)."""
    task["updated_at"] = stamp
    if actor:
        task["updated_by"] = actor


def back_to_todo(task, vocab, actor, stamp, keep_commit=False):
    """The task returns to the first todo-kind status; its attempt's timestamps and actors go (DM-4.4). The commit
    goes too, unless `keep_commit`: an expired lease or a release leaves the work in git, to be finished later."""
    task["status"] = status_for(vocab, "todo")
    touch(task, actor, stamp)
    for field in (
        "started_at",
        "completed_at",
        "skipped_at",
        "started_by",
        "completed_by",
        "agent",
        "review_requested_at",
        "waiting_on",
        "questions",
        "blocked_at",
        "sprint_review",
        "resolution",
        "duplicate_of",
    ):
        task.pop(field, None)
    if not keep_commit:
        task.pop("commit", None)


def add_interval(task, field, start, end, on=None, outcome=None):
    """Append {from, to[, on][, outcome]} to the task's list `field`: `blocks` (with who it waited on) or `reviews`
    (with what the review decided, when it sent the task back to an agent)."""
    interval = {"from": start, "to": end}
    if on:
        interval["on"] = on
    if outcome:
        interval["outcome"] = outcome
    task.setdefault(field, []).append(interval)


def answer_note(questions, answers):
    """One note for an unblocked task: each question with its answer."""
    pairs = []
    for i in range(max(len(questions), len(answers))):
        q = questions[i] if i < len(questions) else None
        a = answers[i] if i < len(answers) else None
        pairs.append(f"{q} → {a}" if q and a else (f"{q} → (no answer)" if q else a))
    return "; ".join(pairs)


def is_agent(actor):
    """Whether an actor is an agent (a Claude Code session, "claude:…"); anyone else is a person."""
    return isinstance(actor, str) and actor.startswith("claude:")


def check_review_mode(task, data, agent):
    """`atm done` by review mode (SR-3.8): an agent finishing an active task whose mode is `blocker` or `async` must
    ask for the review instead. A person may always finish a task."""
    vocab = vocabulary(data)
    mode = effective_review(vocab, task)
    if mode in ("blocker", "async") and is_agent(agent) and kind_of(vocab, task.get("status")) == "active":
        raise AtmError(
            f"{task['id']} is reviewed ({mode}): ask for it with atm task state {task['id']} in_review --note '<what to check>'", 2
        )


# ==== common/fields.py ====
# A task's fields as every layer reads them: the effective priority, who started it.

def priority_of(vocab, task):
    """The task's priority, or the default for a missing or unknown one."""
    ids = [p["id"] for p in vocab["priorities"]]
    return task.get("priority") if task.get("priority") in ids else vocab["default_priority"]


def started_by(task):
    """started_by, or `agent` in files written before v4."""
    return task.get("started_by") or task.get("agent")


# ==== common/types.py ====
# Generated from atm-cli/atm/common/schema/ by atm-cli/build.py: don't edit it, change the schema and run
# `pnpm run build:cli` in dashboard/ (the pre-commit hook does it too). TypedDicts with total=False: the required keys are marked in their comments (Python 3.9 has
# no typing.Required).

StatusEntry = TypedDict(
    "StatusEntry",
    {
        # required
        "id": str,
        "label": str,
        # required; What the status means
        "kind": Literal['todo', 'active', 'review', 'blocked', 'done', 'skipped'],
    },
    total=False,
)

VocabularyTerm = TypedDict(
    "VocabularyTerm",
    {
        # required
        "id": str,
        "label": str,
    },
    total=False,
)

LabelEntry = TypedDict(
    "LabelEntry",
    {
        # required
        "id": str,
        "label": str,
        "description": str,
    },
    total=False,
)

PhaseEntry = TypedDict(
    "PhaseEntry",
    {
        # required
        "id": float,
        "name": str,
    },
    total=False,
)

# How the project pushes (docs/v4/skills-and-workflows.md §1)
Workflow = TypedDict(
    "Workflow",
    {
        # direct or pull-request
        "type": str,
        "base": str,
        "verify": bool,
        "max_rounds": int,
        "commit_template": str,
        # "forbid" or "none", or {"require": [the lines]}: what commit messages end with
        "trailers": object,
    },
    total=False,
)

# When atm suggests archiving, and whether agents may do it (DM-5)
ArchiveSettings = TypedDict(
    "ArchiveSettings",
    {
        "threshold_kb": float,
        "threshold_tasks": float,
        "keep_days": float,
        "auto": bool,
    },
    total=False,
)

Sprint = TypedDict(
    "Sprint",
    {
        # required; The sprint's name
        "id": str,
        "goal": str,
        "state": Literal['planned', 'active', 'closed'],
        "release": str,
        # ISO 8601 UTC
        "start": str,
        # ISO 8601 UTC
        "end": str,
        # YYYY-MM-DD
        "due": str,
        "report": "SprintReport",
    },
    total=False,
)

# What a closed sprint did (SR-2.3)
SprintReport = TypedDict(
    "SprintReport",
    {
        # The tasks done in it
        "done": List[str],
        # The unfinished tasks carried on
        "carried": List[str],
        # Where the carried tasks went
        "to": str,
    },
    total=False,
)

Release = TypedDict(
    "Release",
    {
        # required
        "id": str,
        "name": str,
        "state": Literal['planned', 'released', 'archived'],
        # YYYY-MM-DD
        "target": str,
        # ISO 8601 UTC
        "released_at": str,
    },
    total=False,
)

Component = TypedDict(
    "Component",
    {
        # required
        "id": str,
        "label": str,
        # Globs over its files
        "paths": List[str],
        # Its tasks' default review mode
        "review": str,
    },
    total=False,
)

# A project's meta in tasks.json: its settings, vocabularies, sprints, releases and components (docs/v4/data-model.md §2). Every key is optional and falls back to its default.
Meta = TypedDict(
    "Meta",
    {
        # The project's ID (8 hex characters, set by atm init)
        "id": str,
        # The project's name
        "project": str,
        # One line about the project
        "description": str,
        "created": str,
        # The repository's web address, for commit and PR links
        "repository": str,
        # github, gitlab, bitbucket or gitea, for a host whose name doesn't say
        "repository_type": str,
        # The project's logo, relative to its root
        "logo": str,
        # How long a claim lasts without a heartbeat
        "lease_minutes": float,
        "statuses": List["StatusEntry"],
        "types": List["VocabularyTerm"],
        # Most urgent first
        "priorities": List["VocabularyTerm"],
        # The priority of tasks without one
        "default_priority": str,
        "labels": List["LabelEntry"],
        # Before sprints (atm migrate turns them into sprints)
        "phases": List["PhaseEntry"],
        "workflow": "Workflow",
        "archive": "ArchiveSettings",
        # How many tasks tasks-archive.json holds (DM-5.4)
        "archived": int,
        # The number the next new task gets
        "next_id": int,
        # In order
        "sprints": List["Sprint"],
        # In order
        "releases": List["Release"],
        "components": List["Component"],
        # How a sprint created without a name is named: people or time
        "sprint_names": str,
        # The project's default review mode: blocker, async, batch or none
        "review": str,
    },
    total=False,
)

Note = TypedDict(
    "Note",
    {
        # required; ISO 8601 UTC
        "at": str,
        # Who wrote it
        "by": str,
        # required; handoff, outcome, review, rejection, failure, answer or note
        "kind": str,
        # required; Markdown, at most 500 characters
        "text": str,
    },
    total=False,
)

# The PR that contains the task (atm pr): the task is In PR until merged_at (DM-4.3)
PullRequest = TypedDict(
    "PullRequest",
    {
        # required
        "number": int,
        "url": str,
    },
    total=False,
)

BlockRound = TypedDict(
    "BlockRound",
    {
        # required; ISO 8601 UTC
        "from": str,
        # required; ISO 8601 UTC
        "to": str,
        # user or agent
        "on": str,
    },
    total=False,
)

ReviewRound = TypedDict(
    "ReviewRound",
    {
        # required; ISO 8601 UTC
        "from": str,
        # required; ISO 8601 UTC
        "to": str,
        "outcome": str,
    },
    total=False,
)

SprintHistoryEntry = TypedDict(
    "SprintHistoryEntry",
    {
        # required
        "sprint": str,
        # required; ISO 8601 UTC
        "from": str,
        # ISO 8601 UTC; absent while it is in the sprint
        "to": str,
    },
    total=False,
)

# Before v4: the last failed attempt (now a failure note; still read)
LastFailure = TypedDict(
    "LastFailure",
    {
        "at": str,
        "reason": str,
    },
    total=False,
)

# One task, as tasks.json and tasks-archive.json keep it (docs/v4/data-model.md §4). Unknown fields are kept as they are.
Task = TypedDict(
    "Task",
    {
        # required; Letters, digits, '.', '_' and '-' (T-001)
        "id": str,
        # required
        "name": str,
        # required; A type ID from the vocabulary (meta.types)
        "type": str,
        # required; A status ID from the vocabulary (meta.statuses); atm acts on its kind
        "status": str,
        # Markdown
        "description": str,
        # Before sprints: the task's phase (atm migrate turns phases into sprints)
        "phase": float,
        # A priority ID from the vocabulary ("P0"…"P3" by default); missing means the default priority
        "priority": str,
        # Free-form tags that group tasks across sprints and types, e.g. ["spike-2", "ux"] (DM-4.6)
        "labels": List[str],
        # Task IDs it waits for
        "dependencies": List[str],
        "requirements": List[str],
        "use_cases": List[str],
        # Files it creates or changes
        "files": List[str],
        # ISO 8601 UTC
        "created_at": str,
        # ISO 8601 UTC
        "updated_at": str,
        # ISO 8601 UTC; set when the task moves to in progress
        "started_at": str,
        # ISO 8601 UTC; set when the task is done
        "completed_at": str,
        # ISO 8601 UTC; set when the task is skipped (atm skip)
        "skipped_at": str,
        # Before v4: the agent that started the task. Now started_by (readers fall back to it)
        "agent": str,
        # Who created it (docs/v4/data-model.md §4.1)
        "created_by": str,
        # Who changed it last
        "updated_by": str,
        # Who started it: the claim's owner
        "started_by": str,
        # Who finished it
        "completed_by": str,
        # The SHA of the task's commit
        "commit": str,
        "pr": "PullRequest",
        # ISO 8601 UTC; when that PR was merged (atm merged)
        "merged_at": str,
        # ISO 8601 UTC; when the task asked for a review (atm review). Its work ended here; review time isn't work
        "review_requested_at": str,
        # A blocked task waits on a decision by the user or an agent (atm block --on)
        "waiting_on": str,
        # The open questions of a blocked task
        "questions": List[str],
        # ISO 8601 UTC; when it was blocked
        "blocked_at": str,
        # Past blocks, oldest first, with who each waited on (T-244)
        "blocks": List["BlockRound"],
        # Past reviews, oldest first
        "reviews": List["ReviewRound"],
        # Handoffs, outcomes, review requests, answers, rejections (docs/v4/data-model.md §4)
        "notes": List["Note"],
        "last_failure": "LastFailure",
        # The sprint it is in (a sprint's name, meta.sprints); absent: the backlog
        "sprint": str,
        # Each sprint it was in, from when to when; the open entry has no `to` (SR-2.5)
        "sprint_history": List["SprintHistoryEntry"],
        # The release it ships in; when absent, its sprint's (SR-2.6)
        "release": str,
        # Component IDs (meta.components)
        "components": List[str],
        # How it is reviewed: blocker, async, batch or none
        "review": str,
        # In the review kind: waiting, changes_requested or approved (SR-6.1)
        "review_state": str,
        # ISO 8601 UTC; when the review was decided
        "reviewed_at": str,
        # A batch task done by an agent: pending until the end-of-sprint review, then reviewed (SR-4.4)
        "sprint_review": str,
        # `user`: the work is done together with the user (SR-6.5)
        "with": str,
        # Related tasks (a link that doesn't block), stored on one side and shown on both (SR-2.8)
        "related": List[str],
        # How a finished task ended, when it isn't the default: wont_do, duplicate, obsolete or done (SR-2.8)
        "resolution": str,
        # The task this one duplicates (with the duplicate resolution)
        "duplicate_of": str,
    },
    total=False,
)

# atm's suggestion to archive (CLI-5.1), present when the working tasks are over meta.archive's thresholds (T-372)
ArchiveHint = TypedDict(
    "ArchiveHint",
    {
        # required; How big the working task data is
        "bytes": int,
        # required; How many working tasks are finished
        "finished": int,
        # required; How many of them `atm archive` would move
        "archivable": int,
        # required; Whether agents may archive on their own (meta.archive.auto)
        "auto": bool,
        # required; The hint as atm says it
        "text": str,
    },
    total=False,
)

# A task as stored, and what atm derives for it
TaskView = TypedDict(
    "TaskView",
    {
        # required
        "task": "Task",
        # required
        "derived": "TaskDerived",
    },
    total=False,
)

# What atm-lib works out for a task, so no front end applies atm's rules itself
TaskDerived = TypedDict(
    "TaskDerived",
    {
        # required; The status's kind: todo, active, review, blocked, done or skipped (absent for an unknown status)
        "kind": str,
        # required; To do, and every dependency done (an archived one counts as done)
        "ready": bool,
        # required; A to-do task's dependencies that aren't done yet
        "waiting_for": List[str],
        # Who a blocked task waits on: user or agent
        "waiting_on": str,
        # required; Its priority, or the project's default
        "priority": str,
        # Its review mode: its own, else its component's, else the project's
        "effective_review": str,
        # Its release: its own, else its sprint's
        "effective_release": str,
        # A done task's stage: done, in_pr or merged (DM-4.3)
        "stage": str,
        # required; From the archive
        "archived": bool,
        # What atm finds wrong with the task (T-377); it isn't ready then
        "problems": List[str],
    },
    total=False,
)

DependencyItem = TypedDict(
    "DependencyItem",
    {
        # required
        "id": str,
        # required; Its status (", archived" added for an archived task), or missing
        "status": str,
        # required
        "name": str,
    },
    total=False,
)

# atm deps: what a task waits for, and what waits for it
DepsResult = TypedDict(
    "DepsResult",
    {
        # required
        "id": str,
        # required
        "waits_for": List["DependencyItem"],
        # required
        "waited_on_by": List["DependencyItem"],
    },
    total=False,
)

# Who works on a task, from which working copy, until when
Claim = TypedDict(
    "Claim",
    {
        # required
        "task": str,
        # required
        "agent": str,
        "branch": Optional[str],
        "worktree": Optional[str],
        # required
        "claimed_at": str,
        # required
        "lease_expires_at": str,
        "renewed_at": str,
    },
    total=False,
)

ShownDependency = TypedDict(
    "ShownDependency",
    {
        # required
        "id": str,
        # required; null: no such task
        "status": Optional[str],
        # required
        "archived": bool,
    },
    total=False,
)

# A task in full, as atm show gives it (show_tasks): with its effective priority, its dependencies' statuses, the tasks related to it either way and those that duplicate it
TaskShown = TypedDict(
    "TaskShown",
    {
        # required
        "task": "Task",
        # required
        "archived": bool,
        # required
        "priority": Optional[str],
        # required
        "started_by": Optional[str],
        # required
        "dependencies": List["ShownDependency"],
        # required
        "related": List[str],
        # required
        "duplicated_by": List[str],
    },
    total=False,
)

# What a status change (start_task, done_task, ...) answers
StatusChange = TypedDict(
    "StatusChange",
    {
        # required
        "task": "Task",
        # required; Its status before
        "previous": Optional[str],
        # required; Its claim now: the new one after start, else null
        "claim": Optional["Claim"],
        # required; After done: the tasks now ready
        "unblocked": List[str],
        # required; After done: the archive hint, at most once a day
        "hint": Optional["ArchiveHint"],
        # required; After reopen in a closed sprint: where it is now open, and how to move it (T-330)
        "stranded": Optional[str],
        # required; The claims whose lease ran out on the way (their tasks back to to do)
        "expired": List["Claim"],
        # required
        "view": "TaskShown",
    },
    total=False,
)

# A sprint as the overview shows it: done of its tasks but the skipped
SprintNumbers = TypedDict(
    "SprintNumbers",
    {
        # required
        "id": str,
        # required
        "state": Literal['planned', 'active', 'closed'],
        # required
        "due": Optional[str],
        # required
        "release": Optional[str],
        # required
        "done": int,
        # required; Its tasks but the skipped
        "total": int,
        # required
        "percent": int,
    },
    total=False,
)

# The active sprint for the overview's cards (SR-7.7), with its progress
ActiveSprint = TypedDict(
    "ActiveSprint",
    {
        # required
        "id": str,
        # required
        "state": Literal['planned', 'active', 'closed'],
        # required
        "due": Optional[str],
        # required
        "release": Optional[str],
        # required
        "done": int,
        # required; Its tasks but the skipped
        "total": int,
        # required
        "percent": int,
        # required
        "goal": Optional[str],
    },
    total=False,
)

# A live claim: who works on what
WorkingClaim = TypedDict(
    "WorkingClaim",
    {
        # required
        "task": str,
        # required; The task's name, capped at 80 characters
        "name": str,
        # required
        "agent": Optional[str],
        # required
        "branch": Optional[str],
    },
    total=False,
)

# A task by its ID and name (capped at 80 characters in a summary)
TaskName = TypedDict(
    "TaskName",
    {
        # required
        "id": str,
        # required
        "name": str,
    },
    total=False,
)

# The task completed last
LastDone = TypedDict(
    "LastDone",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "completed_at": str,
    },
    total=False,
)

# How many tasks need the user, by why
NeedsYou = TypedDict(
    "NeedsYou",
    {
        # required; In review, waiting for the reviewer
        "review": int,
        # required
        "blocked_on_user": int,
        # required; To do with the user now (SR-6.6)
        "together": int,
    },
    total=False,
)

# A task that needs the user, and why: what the "need you" chips list
NeedsYouItem = TypedDict(
    "NeedsYouItem",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "reason": Literal['review', 'blocked', 'together'],
    },
    total=False,
)

# The latest PR recorded on the tasks (atm pr)
GitPr = TypedDict(
    "GitPr",
    {
        # required
        "number": int,
        # required; Only a web address (docs/security-review.md F9)
        "url": Optional[str],
        # required
        "state": Literal['open', 'merged'],
    },
    total=False,
)

# The project's git line for its card (T-206)
GitSummary = TypedDict(
    "GitSummary",
    {
        # required; The main worktree's branch
        "branch": Optional[str],
        # required
        "pr": Optional["GitPr"],
    },
    total=False,
)

# A card on the All projects board: what a project's Kanban card shows, plus its column's fields
BoardCard = TypedDict(
    "BoardCard",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "type": Optional[str],
        # required; The task's sprint; null in the backlog
        "sprint": Optional[str],
        # required
        "priority": Optional[str],
        # required
        "labels": List[str],
        # In progress: who works on it
        "by": Optional[str],
        # In progress, review, blocked: since when
        "since": Optional[str],
        # Blocked: who decides
        "waiting_on": Optional[str],
        # Blocked: how many questions
        "questions": int,
        # Done: when
        "completed_at": Optional[str],
        # To do (T-349): done with the user
        "with_user": bool,
        # To do (T-349): the dependencies it waits for
        "waiting_for": List[str],
    },
    total=False,
)

# A project's column: its first cards, and how many there are in all
BoardColumn = TypedDict(
    "BoardColumn",
    {
        # required
        "count": int,
        # required
        "cards": List["BoardCard"],
    },
    total=False,
)

# The project's cards for the All projects board (T-257), by column
ProjectBoard = TypedDict(
    "ProjectBoard",
    {
        # required
        "todo": "BoardColumn",
        # required
        "active": "BoardColumn",
        # required
        "review": "BoardColumn",
        # required
        "blocked": "BoardColumn",
        # required
        "done": "BoardColumn",
    },
    total=False,
)

# The overview card's line, archived tasks included (T-255)
ProjectStats = TypedDict(
    "ProjectStats",
    {
        # required
        "started": Optional[str],
        # required
        "tasks": int,
        # required
        "agent_minutes": int,
    },
    total=False,
)

# A project's numbers for the overview and the project selector (api/projects' summary, atm summary)
ProjectSummary = TypedDict(
    "ProjectSummary",
    {
        # required
        "total": int,
        # required
        "done": int,
        # required
        "skipped": int,
        # required; done / (total - skipped)
        "percent": int,
        # required; null when none is active
        "active_sprint": Optional["ActiveSprint"],
        # required; Up to 4 sprints with tasks, in the project's order: the open ones, then the latest closed; null without sprints
        "recent_sprints": Optional[List["SprintNumbers"]],
        # required
        "agents_working": int,
        # required; Live claims, oldest first
        "working": List["WorkingClaim"],
        # required; Up to 2 tasks the loop takes next, in its order
        "next": List["TaskName"],
        # required
        "last_done": Optional["LastDone"],
        # required
        "needs_you": "NeedsYou",
        # required
        "needs_you_tasks": List["NeedsYouItem"],
        # required
        "git": "GitSummary",
        # required; meta.description, capped at 200 characters
        "description": Optional[str],
        # required
        "last_activity": Optional[str],
        # required; Oldest day first
        "completions_14d": List[int],
        # required
        "board": "ProjectBoard",
        # required; The same, of the active sprint's tasks only; null without an active sprint (T-340)
        "board_active_sprint": Optional["ProjectBoard"],
        # required
        "stats": "ProjectStats",
    },
    total=False,
)

# atm status: the active sprint and its progress
StatusSprint = TypedDict(
    "StatusSprint",
    {
        "id": str,
        "goal": str,
        "due": str,
        "release": str,
        "start": str,
        # required
        "done": int,
        # required; Its tasks but the skipped
        "tasks": int,
        # required; Batch tasks waiting for the end-of-sprint review (SR-4.4)
        "review_pending": List[str],
    },
    total=False,
)

InProgressItem = TypedDict(
    "InProgressItem",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required; The agent holding its claim, else who started it
        "by": Optional[str],
    },
    total=False,
)

InReviewItem = TypedDict(
    "InReviewItem",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "since": Optional[str],
        # In review in another worktree, on this branch (T-204)
        "branch": Optional[str],
    },
    total=False,
)

BlockedItem = TypedDict(
    "BlockedItem",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "waiting_on": Optional[str],
        # required
        "questions": List[str],
        # Blocked in another worktree, on this branch (T-204)
        "branch": Optional[str],
    },
    total=False,
)

WaitingItem = TypedDict(
    "WaitingItem",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required; Its dependencies not done yet
        "on": List[str],
    },
    total=False,
)

LargestTask = TypedDict(
    "LargestTask",
    {
        # required
        "id": str,
        # required
        "bytes": int,
    },
    total=False,
)

# An unfinished task whose sprint is closed (T-330)
OpenInClosedSprint = TypedDict(
    "OpenInClosedSprint",
    {
        # required
        "id": str,
        # required
        "name": str,
        # required
        "status": str,
        # required
        "sprint": str,
    },
    total=False,
)

StatusSuggestions = TypedDict(
    "StatusSuggestions",
    {
        # required
        "archive": "ArchiveHint",
    },
    total=False,
)

# atm status --json: the overview, in the scope's tasks
StatusReport = TypedDict(
    "StatusReport",
    {
        # required; null when none is active
        "sprint": Optional["StatusSprint"],
        # required
        "tasks": int,
        # required
        "by_status": Dict[str, int],
        # required; By sprint ID; backlog for the tasks in none
        "by_sprint": Dict[str, int],
        # required
        "by_type": Dict[str, int],
        # required
        "by_label": Dict[str, int],
        # required
        "in_progress": List["InProgressItem"],
        # required
        "in_review": List["InReviewItem"],
        # required
        "blocked": List["BlockedItem"],
        # required
        "waiting_on_dependencies": List["WaitingItem"],
        # required
        "next": Optional["TaskName"],
        # required; Why there is no next task, as atm next says it
        "next_reason": Optional[str],
        # required
        "file_bytes": int,
        # required
        "largest_tasks": List["LargestTask"],
        # required; How many tasks the archive holds
        "archived": int,
        # required
        "archive_bytes": int,
        # Unfinished tasks left in closed sprints, whatever the scope; absent without any
        "open_in_closed_sprints": List["OpenInClosedSprint"],
        "suggest": "StatusSuggestions",
    },
    total=False,
)

# GET /api/v1/projects/<p>/tasks: the tasks the query's filters keep, each with its derived values (docs/layered-architecture.md §6.2)
TaskViewList = TypedDict(
    "TaskViewList",
    {
        # required; ISO 8601 UTC
        "generated_at": str,
        # required
        "meta": "Meta",
        # required; How big the project's working task data is (tasks.json), for the archive notice
        "bytes": int,
        # required
        "tasks": List["TaskView"],
        # What atm finds wrong with the task file as a whole: entries left out, vocabularies that fell back to their defaults (T-377)
        "problems": List[str],
        "archive_hint": "ArchiveHint",
    },
    total=False,
)


# ==== common/components.py ====
# The rules of components (docs/sprints-and-releases-design.md §2, SR-2.7): the parts of a project (meta.components,
# each with globs over its files), the components a task's files suggest, and the IDs a task may name. Listing,
# defining and changing them is atm-lib's (lib/components.py).

def matches(component, path):
    """Whether `path` (a task's file, relative to the project) is in the component: one of its globs matches. `*`
    and `**` both match across folders (`dashboard/**` is everything under dashboard/)."""
    return any(fnmatchcase(path, glob) for glob in component.get("paths") or [])


def suggest_components(vocab, files):
    """The components the files are in, in the project's order."""
    return [c["id"] for c in vocab["components"] if any(matches(c, f) for f in files if isinstance(f, str))]


def component_ids(vocab, values):
    """The component IDs a task may have: in a project with components, only theirs; otherwise any ID of letters,
    digits, '.', '-' and '_' (agents put them in shell commands). Raises AtmError naming the first bad one."""
    known = [c["id"] for c in vocab["components"]]
    for value in values:
        if not isinstance(value, str) or not RELEASE_ID.fullmatch(value):
            raise AtmError(f"component {value!r}: use letters, digits, '.', '-' and '_' (at most 64)", 2)
        if known and value not in known:
            raise AtmError(f"no component {value!r}; the components: {', '.join(known)}", 2)
    return list(values)


# ==== common/sprint_names.py ====
# Names for sprints created without one (docs/sprints-and-releases-design.md SR-2.9), by meta.sprint_names: a
# historical person's name not yet used in the project (`people`), or the time it was created (`time`).
#
# The names are plain ASCII letters, spaces, hyphens and dots, without apostrophes: agents pass sprint names in shell
# commands, inside single quotes.

PEOPLE = (
    # Scientists and mathematicians
    "Ada Lovelace", "Alan Turing", "Albert Einstein", "Alfred Wegener", "Alexander Fleming", "Alexander von Humboldt",
    "Alhazen", "Andre-Marie Ampere", "Antoine Lavoisier", "Archimedes", "Barbara McClintock",
    "Benjamin Banneker", "Blaise Pascal", "Carl Friedrich Gauss", "Carl Linnaeus", "Caroline Herschel",
    "Charles Babbage", "Charles Darwin", "Chien-Shiung Wu", "Claude Shannon", "Dmitri Mendeleev", "Dorothy Hodgkin",
    "Edmond Halley", "Edwin Hubble", "Emmy Noether", "Enrico Fermi", "Ernest Rutherford", "Euclid", "Evariste Galois",
    "Florence Nightingale", "Galileo Galilei", "George Washington Carver", "Gertrude Elion", "Grace Hopper",
    "Gregor Mendel", "Hedy Lamarr", "Henrietta Leavitt", "Hypatia", "Ibn Sina", "Isaac Newton", "James Clerk Maxwell",
    "Jagadish Chandra Bose", "Johannes Kepler", "John von Neumann", "Jonas Salk", "Katherine Johnson",
    "Leonhard Euler", "Lise Meitner", "Louis Pasteur", "Mae Jemison", "Marie Curie", "Mary Anning",
    "Mary Somerville", "Max Planck", "Michael Faraday", "Muhammad al-Khwarizmi", "Niels Bohr", "Nicolaus Copernicus",
    "Nikola Tesla", "Omar Khayyam", "Pierre Curie", "Rachel Carson", "Richard Feynman", "Robert Hooke",
    "Rosalind Franklin", "Santiago Ramon y Cajal", "Sophie Germain", "Srinivasa Ramanujan", "Stephen Hawking",
    "Subrahmanyan Chandrasekhar", "Tu Youyou", "Tycho Brahe", "Vera Rubin", "Werner Heisenberg", "Zhang Heng",
    "Alessandro Volta", "Anders Celsius", "Christiaan Huygens", "Daniel Bernoulli", "Erwin Schrodinger",
    "Fibonacci", "Gottfried Leibniz", "Henri Poincare", "Hermann von Helmholtz", "Jean-Baptiste Lamarck",
    "Joseph Fourier", "Kurt Godel", "Lord Kelvin", "Maria Goeppert Mayer", "Paul Dirac", "Pierre-Simon Laplace",
    "Ronald Ross", "Seki Takakazu", "Wilhelm Rontgen", "William Harvey", "Annie Jump Cannon", "Cecilia Payne",
    "Edward Jenner", "Hans Christian Orsted", "Joseph Lister", "Lev Landau", "Linus Pauling",
    "Ludwig Boltzmann", "Robert Boyle", "Satyendra Nath Bose", "Wangari Maathai",
    # Engineers and inventors
    "Alexander Graham Bell", "Charles Kettering", "Edith Clarke", "Frank Whittle", "Granville Woods", "Gustave Eiffel",
    "Isambard Brunel", "James Watt", "Johannes Gutenberg", "Karl Benz", "Lillian Gilbreth", "Margaret Hamilton",
    "Nikolaus Otto", "Orville Wright", "Rudolf Diesel", "Samuel Morse", "Thomas Edison", "Wilbur Wright",
    "Yuri Gagarin", "Valentina Tereshkova", "Sally Ride", "Robert Goddard", "Konstantin Tsiolkovsky",
    # Explorers and travellers
    "Amelia Earhart", "Abel Tasman", "Bartolomeu Dias", "Ernest Shackleton", "Ferdinand Magellan", "Fridtjof Nansen",
    "Gertrude Bell", "Ibn Battuta", "Isabella Bird", "James Cook", "Leif Erikson", "Matthew Henson", "Marco Polo",
    "Mary Kingsley", "Roald Amundsen", "Sacagawea", "Tenzing Norgay", "Edmund Hillary", "Zheng He", "Xuanzang",
    "Vasco da Gama", "Robert Falcon Scott", "Nellie Bly", "Freya Stark", "Jacques Cousteau", "Thor Heyerdahl",
    # Writers and poets
    "Agatha Christie", "Anton Chekhov", "Charles Dickens", "Charlotte Bronte", "Emily Bronte", "Emily Dickinson",
    "Franz Kafka", "Fyodor Dostoevsky", "Gabriel Garcia Marquez", "George Eliot", "Hans Christian Andersen",
    "Homer", "Italo Calvino", "Jane Austen", "Jorge Luis Borges", "Jules Verne", "Leo Tolstoy", "Mark Twain",
    "Mary Shelley", "Miguel de Cervantes", "Murasaki Shikibu", "Naguib Mahfouz", "Nazim Hikmet", "Oscar Wilde",
    "Pablo Neruda", "Rabindranath Tagore", "Rumi", "Sappho", "Toni Morrison", "Victor Hugo", "Virginia Woolf",
    "Walt Whitman", "William Shakespeare", "Yunus Emre", "Chinua Achebe", "Dante Alighieri", "Edgar Allan Poe",
    "Geoffrey Chaucer", "Hafez", "Herman Melville", "James Joyce", "Johann Wolfgang von Goethe", "Langston Hughes",
    "Li Bai", "Du Fu", "Matsuo Basho", "Maya Angelou", "Rainer Maria Rilke", "Ursula K. Le Guin", "Wislawa Szymborska",
    "Alexandre Dumas", "Arthur Conan Doyle", "Astrid Lindgren", "Bertolt Brecht", "Gustave Flaubert", "H. G. Wells",
    "Honore de Balzac", "Ivan Turgenev", "Lewis Carroll", "Machado de Assis", "Marcel Proust", "Nikolai Gogol",
    "Orhan Veli", "Robert Louis Stevenson", "Sei Shonagon", "Selma Lagerlof", "Sylvia Plath", "Zora Neale Hurston",
    # Philosophers and thinkers
    "Aristotle", "Baruch Spinoza", "Confucius", "Hannah Arendt", "Ibn Khaldun", "Immanuel Kant",
    "John Locke", "Laozi", "Mary Wollstonecraft", "Michel de Montaigne", "Plato", "Rene Descartes", "Simone de Beauvoir",
    "Socrates", "Thales", "Voltaire", "Zeno", "Adam Smith", "David Hume", "Epicurus", "Mencius", "Pythagoras",
    # Artists and architects
    "Artemisia Gentileschi", "Claude Monet", "Diego Velazquez", "Frida Kahlo", "Georgia OKeeffe", "Hokusai",
    "Johannes Vermeer", "Leonardo da Vinci", "Michelangelo", "Mimar Sinan", "Pablo Picasso", "Paul Cezanne",
    "Rembrandt", "Sandro Botticelli", "Vincent van Gogh", "Wassily Kandinsky", "Zaha Hadid", "Antoni Gaudi",
    "Filippo Brunelleschi", "Frank Lloyd Wright", "Gustav Klimt", "Henri Matisse", "Hilma af Klint", "Paul Klee",
    "Raphael", "Rosa Bonheur", "Titian", "Utagawa Hiroshige", "Berthe Morisot", "Edgar Degas", "Le Corbusier",
    # Composers and musicians
    "Antonio Vivaldi", "Clara Schumann", "Claude Debussy", "Duke Ellington", "Fanny Mendelssohn", "Frederic Chopin",
    "George Gershwin", "Hildegard of Bingen", "Igor Stravinsky", "Johann Sebastian Bach", "Ludwig van Beethoven",
    "Maurice Ravel", "Nina Simone", "Pyotr Tchaikovsky", "Scott Joplin", "Wolfgang Amadeus Mozart", "Ella Fitzgerald",
    "Louis Armstrong", "Bela Bartok", "Edvard Grieg", "Franz Schubert", "Giuseppe Verdi", "Jean Sibelius",
    "Joseph Haydn", "Sergei Rachmaninoff", "Antonin Dvorak", "Billie Holiday", "Miles Davis", "Umm Kulthum",
)

MONTHS = ("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")


def time_name(when):
    """`May 12-10:38pm`: when the sprint was created, in local time."""
    hour = when.hour % 12 or 12
    return f"{MONTHS[when.month - 1]} {when.day}-{hour}:{when.minute:02d}{'am' if when.hour < 12 else 'pm'}"


def generate_sprint_name(naming, taken):
    """A new sprint name by `naming` (meta.sprint_names), not one of `taken` (compared case-insensitively). The time
    is atm's clock in the local time zone (ATM_NOW and TZ in tests); ATM_SEED fixes the random pick, for tests."""
    used = {name.casefold() for name in taken}
    if naming == "time":
        base = time_name(now().astimezone())
        name, n = base, 1
        while name.casefold() in used:
            n += 1
            name = f"{base}-{n}"
        return name
    free, round_ = [p for p in PEOPLE if p.casefold() not in used], 1
    while not free:
        # Every name taken (a project with hundreds of sprints): the same names again, numbered
        round_ += 1
        free = [f"{p} {round_}" for p in PEOPLE if f"{p} {round_}".casefold() not in used]
    seed = os.environ.get("ATM_SEED")
    return (random.Random(seed) if seed else random.Random()).choice(free)


# ==== common/repository.py ====
# Repository web addresses (meta.repository, e.g. https://github.com/o/r), which the dashboard uses to link commits
# and pull requests: a git remote turned into one, and PR links moved when the repository moves.

SCP_REMOTE = re.compile(r"^(?:[\w.-]+@)?([\w.-]+):(?!//)(.+)$")  # git@github.com:o/r.git
URL = re.compile(r"^(https?|ssh|git)://(?:[^@/]+@)?([^/:]+)(?::\d+)?(/.*)?$")


def repository_url(text):
    """The web address of a repository: an http(s) URL as given, or a git remote turned into one
    (git@github.com:o/r.git, ssh://git@github.com/o/r.git, https://github.com/o/r.git -> https://github.com/o/r).
    Credentials, a trailing .git and a trailing slash go. None if it isn't one."""
    text = (text or "").strip()
    m = URL.match(text)
    if m:
        scheme, host, path = m.group(1), m.group(2), (m.group(3) or "")
        web = "https" if scheme in ("ssh", "git") else scheme
    else:
        m = SCP_REMOTE.match(text)
        if not m or "/" in m.group(1):
            return None
        web, host, path = "https", m.group(1), "/" + m.group(2)
    path = re.sub(r"\.git/?$", "", path.rstrip("/")).rstrip("/")
    if not host or "." not in host and host != "localhost" or not path.strip("/"):
        return None
    return f"{web}://{host}{path}"


def move_pr_links(tasks, old, new):
    """Point the tasks' recorded PR links (pr.url) on the `old` address at the `new` one, after the repository moved.
    Only the link changes: no timestamps or actors, so the history stays as it was. Returns how many moved."""
    moved = 0
    for task in tasks:
        pr = task.get("pr") if isinstance(task, dict) else None
        link = pr.get("url") if isinstance(pr, dict) else None
        if isinstance(link, str) and (link == old or link.startswith(old + "/")):
            pr["url"] = new + link[len(old) :]
            moved += 1
    return moved


# ==== host/git.py ====
# Running git, and reading a working copy's HEAD (docs/layered-architecture.md §11.8): the one place that does, for
# both of git's roles, carrying the task files (the JSON-in-git store) and the code's version control (vcs/git.py).

def git(args, cwd):
    """Run git; return stdout stripped, or None if git fails or isn't installed (a question, not a step)."""
    try:
        result = subprocess.run(["git"] + args, cwd=cwd, capture_output=True, text=True)
    except OSError:
        return None
    return result.stdout.strip() if result.returncode == 0 else None


def git_exit(args, cwd):
    """Run git for its exit code alone (a command that answers yes or no that way); None if git isn't installed."""
    try:
        return subprocess.run(["git"] + args, cwd=cwd, capture_output=True).returncode
    except OSError:
        return None


def run_git(args, cwd, strip=True):
    """Run git as a step that must work: return stdout (stripped, unless `strip` is False: a status line starts with
    a space that means something), or raise with git's own message."""
    try:
        result = subprocess.run(["git"] + args, cwd=cwd, capture_output=True, text=True)
    except OSError as e:
        raise AtmFailure(f"git isn't available: {e}")
    if result.returncode != 0:
        message = (result.stderr or result.stdout).strip()
        command = next((a for a in args if not a.startswith("-")), "")
        raise AtmFailure(f"git {command} failed: {message}")
    return result.stdout.strip() if strip else result.stdout


def in_git(folder):
    """Whether the folder is in a git repository: the one way atm asks."""
    return git(["rev-parse", "--git-dir"], folder) is not None


def head_branch(git_dir):
    """The branch checked out per a git directory's HEAD file; a detached HEAD gives its short SHA, no file None."""
    try:
        with open(os.path.join(git_dir, "HEAD"), encoding="utf-8") as f:
            head = f.readline().strip()
    except OSError:
        return None
    if head.startswith("ref: "):
        ref = head[5:]
        return ref[len("refs/heads/") :] if ref.startswith("refs/heads/") else ref
    return head[:7] or None


def checked_out_branch(worktree):
    """What "the branch" means everywhere in atm (a claim's, a project card's, the worktree list's): the branch the
    working copy at `worktree` has checked out, read from its HEAD without running git; a detached HEAD gives its
    short SHA; None outside git. A linked worktree's `.git` is a file naming its git directory."""
    dot_git = os.path.join(worktree, ".git")
    if os.path.isfile(dot_git):
        try:
            with open(dot_git, encoding="utf-8") as f:
                line = f.readline().strip()
        except OSError:
            return None
        if not line.startswith("gitdir: "):
            return None
        return head_branch(os.path.join(worktree, line[len("gitdir: ") :]))
    return head_branch(dot_git)


# ==== db/interface.py ====
# The store: how atm-lib reads and writes a project's data, whatever keeps it (docs/layered-architecture.md §3).
#
# It grows with the migration, one slice at a time (§9): each slice adds the operations it needs. Tasks are the
# dicts of the task file's format (docs/v4/data-model.md §4); a store may hand out its own objects, so atm-lib never
# changes what a read returns.

class Store(Protocol):
    def get_vocabularies(self) -> dict:
        """The project's statuses, types, priorities and the rest of its vocabularies, with the defaults filled in."""
        ...

    def get_settings(self) -> dict:
        """The project's settings: meta without its lists (project, description, id, repository, logo, workflow,
        review, archive, …)."""
        ...

    def list_sprints(self) -> List[dict]:
        """The sprints, in order, checked (each with its state)."""
        ...

    def list_releases(self) -> List[dict]:
        """The releases, in order, checked."""
        ...

    def list_components(self) -> List[dict]:
        """The components, with their globs, checked."""
        ...

    def list_tasks(self) -> List[dict]:
        """The working tasks (not the archived ones), in the project's order."""
        ...

    def get_task(self, task_id: str) -> Optional[dict]:
        """A working task, or None."""
        ...

    def get_archived_task(self, task_id: str) -> Optional[dict]:
        """An archived task, or None."""
        ...

    def list_archived(self) -> List[dict]:
        """The archived tasks."""
        ...

    def read_claims(self) -> dict:
        """The claims and what goes with them: `claims` (by task), `completed`, `waiting` and `resolved` (tasks done,
        waiting or decided in another working copy) and `hints` (when each hint was last shown)."""
        ...

    def sizes(self) -> dict:
        """How big the project's data is, in bytes: `tasks` (the working tasks) and `archive`."""
        ...

    def files_for_commit(self) -> List[str]:
        """The files of the store that travel with a code commit (relative to the repository): tasks.json for the
        JSON-in-git store, none for a store that syncs elsewhere (docs/layered-architecture.md §2)."""
        ...

    def problems(self) -> dict:
        """What a tolerant read found wrong with the data (T-377): {"file": [...], "tasks": {id: [...]}}, in `atm
        check`'s words; empty for a strict read, which refuses such data instead."""
        ...

    def working_copy(self) -> Optional[str]:
        """The working copy this store reads, for a store that has several (git worktrees); None otherwise."""
        ...

    def raw(self) -> dict:
        """The data as stored, unchecked, for `atm check` to report on: {"document" (None when it isn't task data),
        "archive" (the archived tasks), "archive_error" (why the archive can't be read, or None)}."""
        ...

    def change(self, fn: Callable[[Any], Any]) -> Any:
        """Run `fn(work)` (a db.working.Working: the tasks and claims to change) under the store's isolation, and write
        what it changed; nothing when it raises. Returns what `fn` returns, then raises what it deferred. The
        transitional form of `apply` (docs/layered-architecture.md §3.3): `work.changes()` is the list of writes."""
        ...

    def document(self) -> dict:
        """The whole task document (`meta` and `tasks`), for the rules that still take it whole."""
        ...

    def exists(self) -> bool:
        """Whether the project has task data yet (`atm init` creates it)."""
        ...

    def create(self, document: dict) -> None:
        """Store the project's first task data; Conflict if it has some already."""
        ...

    def stamp(self) -> dict:
        """What changes when the data does, for the REST API's ETags: per part ({tasks, archive, claims, head, copies}),
        a value that changes whenever that part does; None for a part that isn't there."""
        ...

    def logo(self, meta: dict) -> Optional[dict]:
        """The project's logo for the dashboard: {path, file, type, source: "set" or "found"}, or None."""
        ...

    def check_logo(self, relative: str) -> None:
        """Refuse (Invalid) a logo the dashboard can't be given."""
        ...

    def working_copies(self) -> List[dict]:
        """The project's working copies, main first ([{path, branch, main}]); [] for a store without them."""
        ...

    def copy_tasks(self, folder: str) -> List[dict]:
        """The tasks as the working copy at `folder` has them."""
        ...

    def copy_store(self, folder: str) -> "Store":
        """The working copy at `folder` as a store of its own, read as far as it can be (tolerant), for working out
        its tasks' derived values on its own data (§11.10 A). AtmFailure when it can't be opened."""
        ...


class Registry(Protocol):
    """The user's list of projects, the ones the dashboard shows (docs/layered-architecture.md §3.2). Each operation
    also says where an unreadable list was moved (`damaged`), or None."""

    def list_projects(self) -> dict:
        """{projects: the entries ({id, name, path, added_at, seen_at, missing}), damaged}."""
        ...

    def add_project(self, target: Optional[str]) -> dict:
        """Register the project in the folder `target`. {result, project, damaged}."""
        ...

    def remove_project(self, target: str) -> dict:
        """Unregister the project with this ID or path. {removed, damaged}."""
        ...

    def prune(self) -> dict:
        """Unregister the projects that are gone. {removed, damaged}."""
        ...

    def entries(self) -> List[dict]:
        """The entries as they are now, read without waiting (atm serve, on every request)."""
        ...

    def stamp(self) -> Any:
        """What changes when the list does."""
        ...


# ==== vcs/interface.py ====
# The code's version control, as atm-lib uses it (docs/layered-architecture.md §2). Git's other role, carrying the
# task files, belongs to the JSON-in-git store, not here. vcs/git.py implements it with git.

class Vcs(Protocol):
    def in_repository(self) -> bool:
        """Whether the project's folder is under version control at all."""
        ...

    def paths(self, files: List[str], cwd: Optional[str] = None) -> List[str]:
        """The files to commit, relative to the repository, from names relative to `cwd` (None: to the repository
        itself); refuses a name that would
        commit more than one file (the whole tree, a directory, a pattern) or a file outside the repository."""
        ...

    def commit(self, files: List[str], message: str) -> str:
        """Commit exactly these files; the new commit's SHA."""
        ...

    def resolve(self, rev: str) -> Optional[str]:
        """A revision's SHA, or None if there is no such commit."""
        ...

    def in_history(self, sha: str) -> bool:
        """Whether the commit is reachable from the current one."""
        ...

    def message(self, sha: str) -> str:
        """A commit's message."""
        ...

    def default_base(self) -> Optional[str]:
        """The remote's default branch."""
        ...

    def remote_url(self) -> Optional[str]:
        """The remote's address."""
        ...

    def branch(self) -> Optional[str]:
        """The checked-out branch."""
        ...

    def file_stamp(self, relative: str) -> Optional[Tuple[int, int]]:
        """(size, nanosecond mtime) of the working copy's file at `relative` (a checked, `/`-separated path), or None
        when it isn't a regular file reached through no symbolic link from the project's root."""
        ...

    def read_file(self, relative: str) -> bytes:
        """The bytes of that file (NotFound when file_stamp would say None)."""
        ...

    def ignored(self, relative: str) -> bool:
        """Whether version control ignores the file (git's ignore rules; a tracked file never is); False outside it."""
        ...

    def tracked(self, relative: str) -> Optional[bool]:
        """Whether version control tracks the file; None outside it."""
        ...

    def head(self) -> Optional[str]:
        """The current commit's SHA; None outside version control or before the first commit."""
        ...

    def changes_under(self, folders: List[str], cwd: Optional[str] = None) -> List[str]:
        """Every changed file under each folder (named relative to `cwd`), relative to the repository: what
        `atm commit --dir` commits (T-405). Invalid for the root, anything outside the repository and the task folder;
        NotFound for a folder with nothing changed."""
        ...

    def submodule_of(self, relative: str) -> Optional[dict]:
        """For a file inside a submodule (the innermost): {path: the submodule's folder, commit: its own HEAD,
        repository: its web address or None}; None for any other file, and outside version control (T-408)."""
        ...


# ==== db/jsongit/gitrun.py ====
# Git as the carrier of the task files (docs/layered-architecture.md §2): whether a task file is tracked. Running git
# itself is host/git.py's (§11.8); git as the code's version control is vcs/git.py.

def is_tracked(root, p):
    return git(["ls-files", "--error-unmatch", "--", rel(root, p)], root) is not None


# ==== db/jsongit/files.py ====
# The JSON-in-git store's files (docs/v4/data-model.md §1): reading tasks.json strictly (every command) or tolerantly
# (showing a malformed file, T-377), and the archive, tasks-archive.json, which the rules look archived tasks up in.

ARCHIVE_FILE = "tasks-archive.json"


def archive_path(paths):
    return os.path.join(paths["dir"], ARCHIVE_FILE)


def load_archive(paths):
    """tasks-archive.json, or an empty archive when there is none (DM-1.2)."""
    p = archive_path(paths)
    if not os.path.isfile(p):
        return {"meta": {}, "tasks": []}
    data = read_json(p)
    if not isinstance(data, dict) or not isinstance(data.get("tasks"), list):
        raise AtmError(f"{p} is not a valid archive (expected an object with a 'tasks' list)")
    return data


def archive_of(paths):
    """A lookup of the archived tasks in the archive of `paths` (`lookup(task_id)` -> task or None), read on the first
    lookup: what the store hands the rules for its project."""
    found = {}

    def lookup(task_id):
        if "tasks" not in found:
            found["tasks"] = {t.get("id"): t for t in load_archive(paths)["tasks"] if isinstance(t, dict)}
        return found["tasks"].get(task_id)

    return lookup


def read_task_file(paths):
    if not os.path.isfile(paths["tasks"]):
        raise AtmError(f"no tasks.json in {paths['dir']}. Run /atm:create-tasks first.")
    data = read_json(paths["tasks"])
    if not isinstance(data, dict) or not isinstance(data.get("tasks"), list):
        raise AtmError(f"{paths['tasks']} is not a valid task file (expected an object with a 'tasks' list)")
    return data


def load_tasks(paths):
    data = read_task_file(paths)
    vocabulary(data)  # an unusable vocabulary stops every command before anything is written
    # Agents put task IDs into shell commands: an ID with shell characters stops every command (atm check lists them)
    for i, task in enumerate(data["tasks"]):
        # Every command looks tasks up by ID and follows dependencies: a malformed task stops them all with a clear
        # message instead of a traceback halfway through (docs/security-review.md F12). atm check lists them all.
        if not isinstance(task, dict) or not isinstance(task.get("id"), str):
            raise AtmError(f"{paths['tasks']}: task #{i + 1} isn't a task (an object with an \"id\"); run atm doctor")
        if not valid_task_id(task["id"]):
            raise AtmError(
                f"{paths['tasks']}: the task ID {task['id']!r} has characters atm doesn't allow (letters, digits, '.', "
                "'_' and '-' only): fix it in the file, then run atm doctor"
            )
        deps = task.get("dependencies", [])
        if not isinstance(deps, list) or not all(isinstance(d, str) for d in deps):
            raise AtmError(f"{paths['tasks']}: {task['id']}'s dependencies aren't a list of IDs; run atm doctor")
    return data


def load_tasks_tolerant(paths):
    """tasks.json read as far as it can be, for showing it (the dashboard): what `load_tasks` refuses is kept where it
    can be, and said. Entries that aren't tasks (no string "id") are left out; a task with an ID atm doesn't allow is
    kept as it is, one whose dependencies aren't a list of IDs with the IDs that can be read (so every task read fits
    the task schema). The data is for reading only. Returns (data, problems): problems as {"file": [...],
    "tasks": {id: [...]}}, in `atm check`'s words. A file that isn't a task file at all is still an error."""
    data = read_task_file(paths)
    problems = {"file": [], "tasks": {}}
    kept = []
    for i, task in enumerate(data["tasks"]):
        if not isinstance(task, dict) or not isinstance(task.get("id"), str):
            problems["file"].append(f"task #{i + 1} isn't a task (an object with an \"id\"): left out")
            continue
        own = []
        if not valid_task_id(task["id"]):
            own.append(f"the ID {task['id']!r} has characters atm doesn't allow (letters, digits, '.', '_' and '-')")
        deps = task.get("dependencies", [])
        if not isinstance(deps, list) or not all(isinstance(d, str) for d in deps):
            # Shown with the IDs that can be read, so what reads it gets a task of the schema's shape
            task["dependencies"] = [d for d in deps if isinstance(d, str)] if isinstance(deps, list) else []
            own.append("its dependencies aren't a list of task IDs (shown with the IDs that can be read)")
        if own:
            problems["tasks"][task["id"]] = own
        kept.append(task)
    data["tasks"] = kept
    # The rules read the vocabularies from meta: what doesn't work is left out of it (the problems say what)
    _, vocabulary_problems, data["meta"] = tolerant_vocabulary(data.get("meta"))
    problems["file"] += vocabulary_problems
    return data, problems


def archive_hint(paths, data):
    """The suggestion to archive (CLI-5.1) for the task file of `paths`, or None (common/model.py archive_hint_for)."""
    return archive_hint_for(data, os.path.getsize(paths["tasks"]) if os.path.isfile(paths["tasks"]) else 0)


# ==== db/jsongit/paths.py ====
# Where a project's task files are: the project root, the task folder, legacy locations (`atm paths` prints them).

LEGACY_DIRS = ["docs", "."]
# The files atm reads and writes in the task folder; a repository can commit any of them as a symlink
FOLDER_FILES = (TASKS_FILE, "tasks-archive.json", ".gitignore", ".gitattributes")


def check_task_folder(root, task_dir):
    """Refuse a task folder atm would read or write outside the project through (docs/security-review.md F7): a
    cloned repository can commit `.agentic-task-manager` or its files as symlinks to anything the user can write,
    such as ~/.zshrc."""
    if os.path.islink(task_dir):
        raise AtmError(f"{task_dir} is a symlink; atm works only on a real folder inside the project")
    if not os.path.isdir(task_dir):
        return
    real_root = os.path.realpath(root)
    if os.path.commonpath([os.path.realpath(task_dir), real_root]) != real_root:
        raise AtmError(f"{task_dir} leads outside the project ({os.path.realpath(task_dir)})")
    for name in FOLDER_FILES:
        if os.path.islink(os.path.join(task_dir, name)):
            raise AtmError(f"{os.path.join(task_dir, name)} is a symlink; atm reads and writes only regular files there")


def project_root(cwd):
    """The git top level; outside git, the nearest directory with a task folder or docs/tasks.json, else cwd."""
    top = git(["rev-parse", "--show-toplevel"], cwd)
    if top:
        return top
    d = cwd
    while True:
        if os.path.isdir(os.path.join(d, TASK_DIR)) or os.path.isfile(os.path.join(d, "docs", TASKS_FILE)):
            return d
        parent = os.path.dirname(d)
        if parent == d:
            return cwd
        d = parent


def find_paths(cwd):
    root = project_root(cwd)
    task_dir = os.path.join(root, TASK_DIR)
    check_task_folder(root, task_dir)
    legacy = None
    for d in LEGACY_DIRS:
        candidate = os.path.normpath(os.path.join(root, d, TASKS_FILE))
        if os.path.isfile(candidate):
            dashboard = os.path.join(os.path.dirname(candidate), DASHBOARD_FILE)
            legacy = {"tasks": candidate, "dashboard": dashboard if os.path.isfile(dashboard) else None}
            break
    tasks = os.path.join(task_dir, TASKS_FILE)
    return {
        "root": root,
        "dir": task_dir,
        "tasks": tasks,
        "tasks_exists": os.path.isfile(tasks),
        "dashboard": os.path.join(task_dir, DASHBOARD_FILE),
        "legacy": legacy,
    }


# ==== db/jsongit/lock.py ====
# The lock directory.
#
# A lock is a directory: mkdir succeeds for exactly one process, on every OS and file system. The holder writes
# owner.json inside it and removes the directory when done. A crashed holder leaves the directory behind, so a waiter
# breaks the lock when its owner is provably gone (same host, pid not running) or it is older than LOCK_STALE. See
# docs/parallel-agents-design.md, section 4.

LOCK_DIR = ".lock"
LOCK_OWNER = "owner.json"
LOCK_TIMEOUT = 10.0  # seconds a command waits for the lock (ATM_LOCK_TIMEOUT overrides)
LOCK_STALE = 60.0  # a lock older than this is abandoned: commands hold it for milliseconds
LOCK_POLL = 0.05


def lock_timeout():
    try:
        return float(os.environ.get("ATM_LOCK_TIMEOUT", LOCK_TIMEOUT))
    except ValueError:
        return LOCK_TIMEOUT


def lock_owner(lock):
    return read_json(os.path.join(lock, LOCK_OWNER))


def lock_age(lock, owner):
    """Seconds since the lock was taken: from owner.json, or the directory itself if owner.json isn't written yet."""
    try:
        since = parse_iso(owner["acquired_at"]).timestamp() if owner else os.stat(lock).st_mtime
    except (OSError, KeyError, TypeError, ValueError):
        return 0.0
    return time.time() - since


def lock_is_stale(lock, owner):
    if owner and owner.get("host") == socket.gethostname() and isinstance(owner.get("pid"), int):
        if not pid_alive(owner["pid"]):
            return True
    return lock_age(lock, owner) > LOCK_STALE


def break_lock(lock, owner):
    """Move a stale lock aside. Returns True if this process broke it.

    Rename first, then check the owner inside the renamed directory (docs/security-review.md F13): checking first and
    renaming after leaves a gap in which another waiter breaks the same stale lock and takes a fresh one, and this
    process would then move the fresh lock aside, leaving two holders. If what was moved isn't the stale lock judged
    here, it goes back."""
    aside = f"{lock}.stale.{os.getpid()}.{os.urandom(4).hex()}"
    try:
        os.rename(lock, aside)
    except OSError:
        return False
    if lock_owner(aside) != owner:
        try:
            os.rename(aside, lock)  # someone else's live lock: put it back
        except OSError:
            pass  # a newer lock is in place already; the moved one was broken by another waiter too
        return False
    shutil.rmtree(aside, ignore_errors=True)
    return True


def describe_owner(owner):
    if not owner:
        return "another process"
    return f"pid {owner.get('pid')} on {owner.get('host')} since {owner.get('acquired_at')}"


@contextlib.contextmanager
def locked(directory, timeout=None):
    """Hold the lock for `directory` (creating it if needed) for the duration of the block. `timeout` (seconds) replaces
    the usual wait; 0 tries once."""
    os.makedirs(directory, exist_ok=True)
    lock = os.path.join(directory, LOCK_DIR)
    deadline = time.time() + (lock_timeout() if timeout is None else timeout)
    while True:
        try:
            os.mkdir(lock)
            break
        except FileExistsError:
            owner = lock_owner(lock)
            if lock_is_stale(lock, owner) and break_lock(lock, owner):
                continue
            if time.time() >= deadline:
                raise AtmError(f"timed out waiting for the lock {lock} (held by {describe_owner(owner)})")
            time.sleep(LOCK_POLL)
    # SIGTERM would otherwise end the process without running the cleanup below
    previous = signal.signal(signal.SIGTERM, lambda signum, frame: sys.exit(128 + signum))
    try:
        write_json_atomic(
            os.path.join(lock, LOCK_OWNER), {"host": socket.gethostname(), "pid": os.getpid(), "acquired_at": now_iso()}
        )
        yield
    finally:
        shutil.rmtree(lock, ignore_errors=True)
        signal.signal(signal.SIGTERM, previous)


# ==== db/jsongit/worktrees.py ====
# The JSON-in-git store's working copies: a clone's git worktrees, each with its own tasks.json (the main worktree's
# is the project's; the others show on the board as worktree copies, `worktrees.json`). Read from the git directory,
# without running git where it can, so the server can ask on every poll.

def main_worktree(root):
    """The main worktree's folder for a project root (itself unless it is a linked git worktree)."""
    common = git(["rev-parse", "--git-common-dir"], root)
    if common:
        common = os.path.normpath(os.path.join(root, common))
        if os.path.basename(common) == ".git":
            return os.path.dirname(common)
    return root


def worktrees(root):
    """The git worktrees of the project whose main worktree is `root`, main first, as `git worktree list` has them:
    [{path, branch, main}]. Linked worktrees whose folder is gone are left out; outside git (or a bare repository) the
    list is empty."""
    git_dir = os.path.join(root, ".git")
    if not os.path.isdir(git_dir):
        return []
    found = [{"path": root, "branch": head_branch(git_dir), "main": True}]
    linked = os.path.join(git_dir, "worktrees")
    try:
        names = sorted(os.listdir(linked))
    except OSError:
        names = []
    for name in names:
        admin = os.path.join(linked, name)
        try:
            with open(os.path.join(admin, "gitdir"), encoding="utf-8") as f:
                path = os.path.dirname(f.readline().strip())  # the worktree's .git file
        except OSError:
            continue
        if path and os.path.isdir(path):
            found.append({"path": path, "branch": head_branch(admin), "main": False})
    return found


# ==== db/jsongit/claims.py ====
# The JSON-in-git store's claims store: which agent works on which task until when, what was finished, waits or was
# decided in another worktree, and when hints were last shown (docs/parallel-agents-design.md). It lives in the git
# common directory, so all worktrees of a clone see it; tasks.json itself is per worktree. Lock order: the task folder,
# then the claims store, so two commands can never wait on each other. The rules on it are atm-lib's (lib/loop.py).

CLAIMS_STORE = "claims.json"  # in <git common dir>/atm/, shared by all worktrees of a clone
CLAIMS_STORE_NO_GIT = "claims-store.json"  # in the task folder, outside git
CLAIMS_SNAPSHOT = "claims.json"  # in the task folder (gitignored): what the dashboard reads


# The git common directory of each working copy, found once per process (it runs git; atm serve asks on every poll)
_common_dirs = {}


def claims_dir(paths):
    root = paths["root"]
    if root not in _common_dirs:
        _common_dirs[root] = git(["rev-parse", "--git-common-dir"], root)
    common = _common_dirs[root]
    if common:
        return os.path.join(os.path.normpath(os.path.join(root, common)), "atm")
    return paths["dir"]


def claims_store_path(paths):
    directory = claims_dir(paths)
    return os.path.join(directory, CLAIMS_STORE if directory != paths["dir"] else CLAIMS_STORE_NO_GIT)


@contextlib.contextmanager
def locked_with_claims(paths):
    """Hold the task folder's lock and, if it is elsewhere, the claims store's lock (always in this order)."""
    with contextlib.ExitStack() as stack:
        stack.enter_context(locked(paths["dir"]))
        if claims_dir(paths) != paths["dir"]:
            stack.enter_context(locked(claims_dir(paths)))
        yield


def read_store(paths):
    """The shared store: live `claims`, and `completed` tasks (done in some worktree; its tasks.json may be the only
    one that says so until the branches merge, so `next` must not hand those tasks out again elsewhere); `waiting`
    tasks (blocked or sent for review in some worktree, likewise); `resolved` ones (answered, passed, rejected or
    skipped in another worktree than the one they wait in, to be applied there); and `hints`, when each hint was last
    shown (so `done` repeats one at most once a day)."""
    data = read_json(claims_store_path(paths))
    data = data if isinstance(data, dict) else {}

    def section(key):
        return data.get(key) if isinstance(data.get(key), dict) else {}

    return {
        "claims": data.get("claims") or {},
        "completed": data.get("completed") or {},
        "waiting": section("waiting"),
        "resolved": section("resolved"),
        "hints": section("hints"),
    }


def write_store(paths, store):
    os.makedirs(claims_dir(paths), exist_ok=True)
    write_json_atomic(claims_store_path(paths), store)
    write_claims_snapshot(paths, store["claims"])


def write_claims_snapshot(paths, claims):
    """The gitignored copy the dashboard polls: live claims, as a list."""
    if os.path.isdir(paths["dir"]):
        snapshot = {"generated_at": now_iso(), "claims": sorted(claims.values(), key=lambda c: c["task"])}
        write_json_atomic(os.path.join(paths["dir"], CLAIMS_SNAPSHOT), snapshot)


# ==== db/jsongit/logo.py ====
# The project's logo for the dashboard: the image meta.logo names (a path relative to the project root), or else one
# found in the usual places. atm serve serves it at /p/<id>/logo (docs/multi-project-api.md).

LOGO_TYPES = {
    ".svg": "image/svg+xml",
    ".png": "image/png",
    ".webp": "image/webp",
    ".ico": "image/x-icon",
}
MAX_LOGO_BYTES = 512 * 1024
# Where a logo usually is, first match wins: logo.svg/logo.png in the root, then .github/logo.*, public/favicon.* and
# assets/logo.*, each in the order of LOGO_TYPES
LOGO_CANDIDATES = ["logo.svg", "logo.png"] + [
    f"{base}{ext}" for base in (".github/logo", "public/favicon", "assets/logo") for ext in LOGO_TYPES
]


def logo_file(root, relative):
    """(absolute path, content type) of the image at `relative` in the project at `root`. AtmError (exit code 2) if
    it isn't a supported image type, is outside the project (symbolic links followed), is missing or too large."""
    if not isinstance(relative, str) or not relative.strip():
        raise AtmError("no logo path given", 2)
    content_type = LOGO_TYPES.get(os.path.splitext(relative)[1].lower())
    if not content_type:
        raise AtmError(f"{relative}: a logo is an SVG, PNG, WebP or ICO image", 2)
    real_root = os.path.realpath(root)
    full = os.path.realpath(os.path.join(real_root, relative))
    if os.path.commonpath([real_root, full]) != real_root:
        raise AtmError(f"{relative}: the logo must be inside the project folder", 2)
    if not os.path.isfile(full):
        raise AtmError(f"{relative}: no such file", 2)
    if os.path.getsize(full) > MAX_LOGO_BYTES:
        raise AtmError(f"{relative}: the logo is larger than {MAX_LOGO_BYTES // 1024} KB", 2)
    return full, content_type


def find_logo(root, meta):
    """(path relative to root, absolute path, content type, how it was found: "set" or "found") of the project's
    logo, or None. meta.logo wins when it is set (a logo it names that is unusable means no logo); otherwise the first
    usable LOGO_CANDIDATES file."""
    if meta.get("logo"):
        try:
            return (meta["logo"], *logo_file(root, meta["logo"]), "set")
        except AtmError:
            return None
    for candidate in LOGO_CANDIDATES:
        try:
            return (candidate, *logo_file(root, candidate), "found")
        except AtmError:
            continue
    return None


# ==== db/jsongit/merge.py ====
# The JSON-in-git store's merge driver: merging the task files task by task.
#
# Registered as a git merge driver (`tasks.json merge=atm`), so branches that changed tasks.json merge task by task
# instead of line by line. Design: docs/parallel-agents-design.md, option E and layer L3.

# What `atm` means is not in this file: git config can't be committed, so each clone registers the driver in its own
# .git/config (merge.atm.driver), which the atm skills do on every run. A clone without it merges tasks.json normally.
GITATTRIBUTES_LINES = (
    "# 'atm' is defined in each clone's git config (merge.atm.driver), registered by the atm plugin on every run.\n"
    "# Without it, git merges tasks.json line by line as usual.\n"
    "tasks.json merge=atm\n"
)
# Fields that move with the status: they come from the side whose progress wins
PROGRESS_FIELDS = (
    "status",
    "updated_at",
    "started_at",
    "completed_at",
    "skipped_at",
    "last_failure",
    "agent",
    "started_by",
    "completed_by",
    "updated_by",
    "commit",
    "review_requested_at",
    "review_state",
    "reviewed_at",
    "sprint_review",
    "waiting_on",
    "questions",
    "blocked_at",
)
# How far along a status's kind is (DM-3.3); a status without a kind ranks like todo
KIND_RANK = {"todo": 0, "active": 1, "blocked": 1, "review": 1, "done": 2, "skipped": 2}
MISSING = object()


def merge_vocabulary(*versions):
    """The vocabulary to rank statuses by: this branch's, else the other's, else the defaults. A merge never fails
    over a vocabulary; the next atm command reports it."""
    for data in versions:
        try:
            return vocabulary(data)
        except AtmError:
            continue
    return vocabulary({})


def merge_value(base, ours, theirs):
    """Three-way merge of one value: (value, conflict). On conflict, ours is kept."""
    if ours == theirs:
        return ours, False
    if ours == base:
        return theirs, False
    if theirs == base:
        return ours, False
    return ours, True


def progress_winner(ours, theirs, vocab):
    """Both sides moved a task along: the one further along wins, then the newer update, then ours."""
    rank = lambda task: KIND_RANK.get(kind_of(vocab, task.get("status")), 0)
    rank_o, rank_t = rank(ours), rank(theirs)
    if rank_o != rank_t:
        return ours if rank_o > rank_t else theirs
    return theirs if str(theirs.get("updated_at", "")) > str(ours.get("updated_at", "")) else ours


def merge_notes(ours, theirs):
    """Notes only grow: both sides' notes, each once, oldest first."""
    merged = list(ours) + [n for n in theirs if n not in ours]
    return sorted(merged, key=lambda n: str(n.get("at", "")) if isinstance(n, dict) else "")


def merge_intervals(ours, theirs):
    """Past blocks and reviews only grow: both sides', one per start, oldest first."""
    starts = {i.get("from") for i in ours if isinstance(i, dict)}
    merged = list(ours) + [i for i in theirs if isinstance(i, dict) and i.get("from") not in starts]
    return sorted(merged, key=lambda i: str(i.get("from", "")) if isinstance(i, dict) else "")


def merge_history(ours, theirs):
    """A task's sprint_history only grows: one entry per start (`from`); an entry ended on either side (`to`) is kept
    ended, so a sprint change on one branch closes the entry the other branch still has open."""
    by_start = {}
    for entry in list(ours) + list(theirs):
        if not isinstance(entry, dict):
            continue
        known = by_start.get(entry.get("from"))
        if known is None or ("to" in entry and "to" not in known):
            by_start[entry.get("from")] = entry
    return sorted(by_start.values(), key=lambda e: str(e.get("from", "")))


# meta lists merged item by item, by ID, like tasks (docs/sprints-and-releases-design.md SR-2.4), with how far along
# each state is: changed on both sides, the further state wins
META_LISTS = {
    "sprints": {"planned": 0, "active": 1, "closed": 2},
    "releases": {"planned": 0, "released": 1, "archived": 2},
    "components": None,
}
# Sprints are named, and two names that differ only in case are the same sprint (SR-2.3)
item_key = lambda name, item: (
    item.get("id").casefold() if name == "sprints" and isinstance(item.get("id"), str) else item.get("id")
)


def merge_items(name, base, ours, theirs):
    """Merge three versions of a meta list of {id, ...} items. Returns (items, conflict descriptions)."""
    rank = META_LISTS[name]
    keyed = lambda items: {item_key(name, i): i for i in items if isinstance(i, dict)} if isinstance(items, list) else {}
    b, o, t = keyed(base), keyed(ours), keyed(theirs)
    items, conflicts = [], []
    for key, item in o.items():
        if key not in b:
            other = t.get(key)
            if other is not None and other != item:
                conflicts.append(f"meta.{name}: {item.get('id')!r} was added on both branches, differently")
            items.append(item)
        elif key not in t:
            if item != b[key]:
                conflicts.append(f"meta.{name}: {item.get('id')!r} changed on this branch, deleted on the other")
                items.append(item)
        else:
            merged, fields = merge_item(b[key], item, t[key], rank)
            items.append(merged)
            if fields:
                conflicts.append(f"meta.{name}: both branches changed {item.get('id')!r} ({', '.join(fields)})")
    for key, item in t.items():
        if key not in o and (key not in b or item != b[key]):
            if key in b:
                conflicts.append(f"meta.{name}: {item.get('id')!r} deleted on this branch, changed on the other")
            items.append(item)
    return items, conflicts


def merge_item(base, ours, theirs, rank):
    """One sprint, release or component changed on both sides: field by field; where both changed a field and both
    changed the state, the side further along wins it (a sprint closed on one branch stays closed with its report);
    otherwise ours, as a conflict. Returns (item, names of conflicting fields)."""
    winner = None
    if rank and ours.get("state") != base.get("state") and theirs.get("state") != base.get("state"):
        ours_rank, theirs_rank = rank.get(ours.get("state"), 0), rank.get(theirs.get("state"), 0)
        if ours_rank != theirs_rank:
            winner = ours if ours_rank > theirs_rank else theirs
    merged, conflicts = {}, []
    for key in list(ours) + [k for k in theirs if k not in ours] + [k for k in base if k not in ours and k not in theirs]:
        value, conflict = merge_value(base.get(key, MISSING), ours.get(key, MISSING), theirs.get(key, MISSING))
        if conflict and winner is not None:
            value, conflict = winner.get(key, MISSING), False
        if conflict:
            conflicts.append(key)
        if value is not MISSING:
            merged[key] = value
    return merged, conflicts


def merge_set(base, ours, theirs):
    """A list merged as a set (`related`, SR-2.8): what either side added stays, what either side removed goes, in
    ours' order, then theirs' additions. MISSING when nothing is left."""
    as_list = lambda v: v if isinstance(v, list) else []
    base, ours, theirs = as_list(base), as_list(ours), as_list(theirs)
    removed = (set(base) - set(ours)) | (set(base) - set(theirs))
    merged = [v for v in ours if v not in removed]
    merged += [v for v in theirs if v not in removed and v not in merged]
    return merged or MISSING


def merge_task(base, ours, theirs, vocab):
    """Merge one task changed on both sides. Returns (task, names of conflicting fields)."""
    changed = lambda side: any(side.get(f, MISSING) != base.get(f, MISSING) for f in PROGRESS_FIELDS)
    if not changed(theirs):
        progress = ours
    elif not changed(ours):
        progress = theirs
    else:
        progress = progress_winner(ours, theirs, vocab)
    keys = list(ours) + [k for k in theirs if k not in ours] + [k for k in base if k not in ours and k not in theirs]
    merged, conflicts = {}, []
    for key in keys:
        if key in PROGRESS_FIELDS:
            value = progress.get(key, MISSING)
        elif key == "notes":
            value = merge_notes(ours.get(key, []), theirs.get(key, []))
        elif key in ("blocks", "reviews"):
            value = merge_intervals(ours.get(key, []), theirs.get(key, []))
        elif key == "sprint_history":
            value = merge_history(ours.get(key, []), theirs.get(key, []))
        elif key == "related":
            value = merge_set(base.get(key, []), ours.get(key, []), theirs.get(key, []))
        else:
            value, conflict = merge_value(base.get(key, MISSING), ours.get(key, MISSING), theirs.get(key, MISSING))
            if conflict and key == "phase" and ("sprint" in ours or "sprint" in theirs):
                # One side has been migrated to sprints (SR-8.5): its sprint stands for the phase
                value, conflict = MISSING, False
            if conflict:
                conflicts.append(key)
        if value is not MISSING:
            merged[key] = value
    return merged, conflicts


def next_free_id(task_id, used):
    parts = split_id(task_id)
    if not parts:
        n = 2
        while f"{task_id}-{n}" in used:
            n += 1
        return f"{task_id}-{n}"
    prefix, digits = parts
    numbers = [int(x[len(prefix) :]) for x in used if x.startswith(prefix) and x[len(prefix) :].isdigit()]
    return prefix + str(max(numbers) + 1).zfill(len(digits))


def merge_task_files(base, ours, theirs):
    """Merge three versions of tasks.json. Returns (merged data, list of conflict descriptions)."""
    by_id = lambda data: {t.get("id"): t for t in data.get("tasks", [])}
    vocab = merge_vocabulary(ours, theirs)
    base_tasks, our_tasks, their_tasks = by_id(base), by_id(ours), by_id(theirs)
    used = set(base_tasks) | set(our_tasks) | set(their_tasks)
    conflicts, merged_tasks, renamed, theirs_new = [], [], {}, []

    for task in ours.get("tasks", []):
        task_id = task.get("id")
        b, t = base_tasks.get(task_id), their_tasks.get(task_id)
        if b is None:
            merged_tasks.append(task)  # added here (if added there too, theirs is renumbered below)
        elif t is None:
            if task != b:
                conflicts.append(f"{task_id}: changed on this branch, deleted on the other")
                merged_tasks.append(task)
        else:
            merged, fields = merge_task(b, task, t, vocab)
            merged_tasks.append(merged)
            if fields:
                conflicts.append(f"{task_id}: both branches changed {', '.join(fields)}")

    for task in theirs.get("tasks", []):
        task_id = task.get("id")
        if task_id in base_tasks:
            if task_id not in our_tasks and task != base_tasks[task_id]:
                conflicts.append(f"{task_id}: deleted on this branch, changed on the other")
                merged_tasks.append(task)
            continue
        if task_id in our_tasks:
            if our_tasks[task_id] == task:
                continue  # the same task added on both sides
            new_id = next_free_id(task_id, used)
            used.add(new_id)
            renamed[task_id] = new_id
            task = {**task, "id": new_id}
        theirs_new.append(task)
    for task in theirs_new:
        if renamed and task.get("dependencies"):
            task = {**task, "dependencies": [renamed.get(d, d) for d in task["dependencies"]]}
        merged_tasks.append(task)

    result = {}
    for key in list(ours) + [k for k in theirs if k not in ours]:
        if key == "tasks":
            result[key] = merged_tasks
        elif key == "meta" and isinstance(ours.get(key), dict) and isinstance(theirs.get(key), dict):
            b, o, t = base.get(key) or {}, ours[key], theirs[key]
            meta = {}
            for k in list(o) + [k for k in t if k not in o]:
                if k in META_LISTS:
                    value, item_conflicts = merge_items(k, b.get(k, []), o.get(k, []), t.get(k, []))
                    conflicts.extend(item_conflicts)
                else:
                    value, _ = merge_value(b.get(k, MISSING), o.get(k, MISSING), t.get(k, MISSING))
                if value is not MISSING:
                    meta[k] = value
            if "sprints" in meta:
                meta.pop("phases", None)  # a project migrated on either side has no phases left (SR-8.5)
            result[key] = meta
        else:
            result[key] = ours.get(key, theirs.get(key))
    return result, conflicts


def merge_files(base_path, ours_path, theirs_path):
    """What git's merge driver does (`atm merge-driver %O %A %B`): merge the three versions task by task and write the
    result over `ours_path`. Returns the conflicts (this branch's version was kept for each)."""
    versions = [read_json(p) for p in (base_path, ours_path, theirs_path)]
    if versions[0] is None and os.path.exists(base_path) and os.path.getsize(base_path) == 0:
        versions[0] = {"tasks": []}  # the file was added on both branches
    if not all(isinstance(v, dict) and isinstance(v.get("tasks", []), list) for v in versions):
        raise AtmError("tasks.json merge: one side is not valid JSON; resolve the conflict by hand")
    merged, conflicts = merge_task_files(*versions)
    write_json_atomic(ours_path, merged)
    return conflicts


def launcher_command():
    launcher = os.path.join(SCRIPT_DIR, "atm").replace("'", "'\\''")
    return f"'{launcher}' db merge-driver %O %A %B"


def ensure_merge_driver(paths):
    """In git: `tasks.json merge=atm` in the task folder's .gitattributes (staged when added), and the driver
    registered in the clone's config, pointing at this plugin's launcher. Returns True if .gitattributes changed."""
    root = paths["root"]
    if not in_git(root):
        return False
    for key, value in (("merge.atm.name", "atm: merge tasks.json task by task"), ("merge.atm.driver", launcher_command())):
        if git(["config", "--get", key], root) != value:
            git(["config", key, value], root)
    attributes = os.path.join(paths["dir"], ".gitattributes")
    try:
        with open(attributes, encoding="utf-8") as f:
            text = f.read()
    except OSError:
        text = ""
    if "tasks.json merge=atm" in text.splitlines():
        return False
    os.makedirs(paths["dir"], exist_ok=True)
    with open(attributes, "w", encoding="utf-8") as f:
        f.write(text + ("" if not text or text.endswith("\n") else "\n") + GITATTRIBUTES_LINES)
    git(["add", "--", rel(root, attributes)], root)
    return True


ARCHIVE_ATTRIBUTES_LINE = "tasks-archive.json merge=atm\n"


def ensure_archive_merge(paths):
    """In git: the archive merges task by task too. Added when a project first archives, so projects that never do
    see no change. Returns True if .gitattributes changed (staged)."""
    root = paths["root"]
    if not in_git(root):
        return False
    ensure_merge_driver(paths)  # the driver itself, and the tasks.json line
    attributes = os.path.join(paths["dir"], ".gitattributes")
    with open(attributes, encoding="utf-8") as f:
        text = f.read()
    if ARCHIVE_ATTRIBUTES_LINE.strip() in text.splitlines():
        return False
    with open(attributes, "w", encoding="utf-8") as f:
        f.write(text + ("" if text.endswith("\n") else "\n") + ARCHIVE_ATTRIBUTES_LINE)
    git(["add", "--", rel(root, attributes)], root)
    return True


# ==== db/jsongit/migrate.py ====
# `atm migrate`, the JSON-in-git store's migrations of old layouts: move a legacy tasks.json into the task folder and
# set the folder up (.gitignore, merge driver), clean up after per-project dashboards (0.10.0-0.14.0: the dashboard now
# comes from the plugin, served by `atm serve` for all projects), and bring old task data up to date (finish times,
# phases to sprints, `with: user`).

# Runtime files in the task folder that must not be committed (the dashboard and server files are from before
# 0.15.0; they stay listed so leftovers stay out of git)
IGNORED = ["tasks-dashboard.html", ".server.json", ".server.log", "claims.json", "claims-store.json", ".lock/"]
OLD_SERVER_RECORD = ".server.json"
OLD_SERVER_LOG = ".server.log"


def written_by_atm(path):
    """Whether an old server record is one atm wrote here: owned by this user and not tracked by git (atm always kept
    it out of git). A record that came with the repository is only deleted: atm never contacts its port or signals its
    pid (docs/security-review.md F14)."""
    try:
        if os.stat(path).st_uid != os.getuid():
            return False
    except (OSError, AttributeError):  # missing, or no uids (Windows)
        return False
    return not is_tracked(os.path.dirname(path), path)


def clean_up_old_dashboard(task_dir):
    """Per-project dashboards (0.10.0-0.14.0): delete the project's copy of the dashboard, stop its server if one still
    runs (only when whoami confirms it serves this folder), and remove the server's record and log. Returns what was
    done, or None when there was nothing left."""
    removed = False
    copy = os.path.join(task_dir, DASHBOARD_FILE)
    if os.path.isfile(copy):
        os.remove(copy)
        removed = True
    stopped = None
    record_path = os.path.join(task_dir, OLD_SERVER_RECORD)
    record = read_json(record_path) if written_by_atm(record_path) else None
    if record is not None:
        info = whoami(record.get("port")) if isinstance(record, dict) else None
        if info and info.get("folder") == task_dir and info.get("pid") == record.get("pid"):
            try:
                os.kill(record["pid"], signal.SIGTERM)
                stopped = record["pid"]
            except OSError:
                pass
    had_files = False
    for name in (OLD_SERVER_RECORD, OLD_SERVER_LOG):
        p = os.path.join(task_dir, name)
        if os.path.exists(p):
            os.remove(p)
            had_files = True
    if not (removed or stopped or had_files):
        return None
    return {"dashboard": removed, "server": stopped}


FINISH_FIELDS = {"done": "completed_at", "skipped": "skipped_at"}


def missing_finish_times(tasks, vocab):
    """The finished tasks without their finish time (done ones from before completed_at existed; skipped ones from
    before skipped_at existed), with the field each one needs."""
    result = []
    for task in tasks:
        field = FINISH_FIELDS.get(kind_of(vocab, task.get("status"))) if isinstance(task, dict) else None
        if field and not task.get(field) and task.get("updated_at"):
            result.append((task, field))
    return result


def backfill_finish_times(paths):
    """Give finished tasks without a finish time their updated_at, which is what the dashboard showed for them anyway,
    so a later edit (which moves updated_at) doesn't move them in time. In tasks.json and the archive; once per task;
    updated_at and updated_by stay. Returns how many tasks were filled in."""
    archive = archive_path(paths)
    try:
        data = load_tasks(paths)
        vocab = vocabulary(data)
        if not missing_finish_times(data["tasks"], vocab) and not missing_finish_times(load_archive(paths)["tasks"], vocab):
            return 0
    except AtmError:
        return 0  # an unreadable file: the command the skill runs next reports the problem
    filled = 0
    with locked(paths["dir"]):
        data = load_tasks(paths)
        files = [(paths["tasks"], data)] + ([(archive, load_archive(paths))] if os.path.isfile(archive) else [])
        for path, contents in files:
            missing = missing_finish_times(contents["tasks"], vocab)
            for task, field in missing:
                task[field] = task["updated_at"]
            if missing:
                write_json_atomic(path, contents)
                filled += len(missing)
    return filled


FINISHED_KINDS = ("done", "skipped")


def task_phase(task):
    """The task's phase (a number), or None: a task without one goes to the backlog."""
    phase = task.get("phase") if isinstance(task, dict) else None
    if not isinstance(phase, (int, float)) or isinstance(phase, bool):
        return None
    return int(phase) if float(phase).is_integer() else phase


def phase_sprint_names(meta, tasks):
    """{phase: sprint name} for every phase with tasks or a name, in phase order: its name, or "Phase n" when
    unnamed, with " (phase n)" added to a name already taken (the names the dashboard shows for unmigrated projects,
    dashboard/src/lib/sprints.ts)."""
    named = {}
    for p in meta.get("phases") or []:
        if isinstance(p, dict) and task_phase({"phase": p.get("id")}) is not None:
            name = p.get("name").strip()[:MAX_SPRINT_NAME].rstrip() if isinstance(p.get("name"), str) else ""
            if name:
                named[task_phase({"phase": p["id"]})] = name
    phases = set(named) | {task_phase(t) for t in tasks} - {None}
    names, taken = {}, set()
    for phase in sorted(phases):
        name = named.get(phase) or f"Phase {phase}"
        if name.casefold() in taken:
            suffix = f" (phase {phase})"
            name = name[: MAX_SPRINT_NAME - len(suffix)].rstrip() + suffix
        taken.add(name.casefold())
        names[phase] = name
    return names


def sprint_from_phase(name, tasks, vocab):
    """The sprint a phase becomes: closed when all its tasks are finished (start, end and report from them), else
    planned (SR-8.1)."""
    finished = bool(tasks) and all(kind_of(vocab, t.get("status")) in FINISHED_KINDS for t in tasks)
    if not finished:
        return {"id": name, "state": "planned"}
    sprint = {"id": name, "state": "closed"}
    starts = [t["started_at"] for t in tasks if isinstance(t.get("started_at"), str)]
    ends = [
        t.get("completed_at") or t.get("skipped_at") or t.get("updated_at")
        for t in tasks
        if isinstance(t.get("completed_at") or t.get("skipped_at") or t.get("updated_at"), str)
    ]
    if starts:
        sprint["start"] = min(starts)
    if ends:
        sprint["end"] = max(ends)
    sprint["report"] = {
        "done": [t["id"] for t in tasks if kind_of(vocab, t.get("status")) == "done" and "id" in t],
        "carried": [],
    }
    return sprint


def phases_to_sprints(data, archived, vocab, stamp):
    """Turn phases into sprints in tasks.json's data and the archive's tasks, in place (SR-8.1). A project that already
    has meta.sprints only gets its leftover phases converted (a task merged in from an unmigrated branch, SR-8.5).
    Returns the sprints it added, or [] when there was nothing to convert."""
    meta = data.setdefault("meta", {})
    tasks = [t for t in list(data["tasks"]) + list(archived) if isinstance(t, dict)]
    if not meta.get("phases") and not any("phase" in t for t in tasks):
        return []
    names = phase_sprint_names(meta, tasks)
    sprints = meta.get("sprints") if isinstance(meta.get("sprints"), list) else []
    existing = {s["id"].casefold() for s in sprints if isinstance(s, dict) and isinstance(s.get("id"), str)}
    added = []
    for phase, name in names.items():
        if name.casefold() in existing:
            continue
        sprint = sprint_from_phase(name, [t for t in tasks if task_phase(t) == phase], vocab)
        sprints.append(sprint)
        added.append(sprint)
    meta["sprints"] = sprints
    meta.pop("phases", None)
    by_name = {s["id"].casefold(): s["id"] for s in sprints if isinstance(s, dict) and isinstance(s.get("id"), str)}
    for task in tasks:
        phase = task_phase(task)
        task.pop("phase", None)
        if phase is None or task.get("sprint"):
            continue
        task["sprint"] = by_name[names[phase].casefold()]
        task.setdefault("sprint_history", []).append(
            {"sprint": task["sprint"], "from": task.get("created_at") if isinstance(task.get("created_at"), str) else stamp}
        )
    return added


def migrate_phases(paths):
    """phases_to_sprints on the files under the lock, staging them in git (not committing). Returns the sprints it
    added, or None when there was nothing to convert."""
    archive = archive_path(paths)
    try:
        data = load_tasks(paths)
        vocab = vocabulary(data)
        archive_data = load_archive(paths)
        if not data.get("meta", {}).get("phases") and not any(
            isinstance(t, dict) and "phase" in t for t in list(data["tasks"]) + list(archive_data["tasks"])
        ):
            return None
    except AtmError:
        return None  # an unreadable file: the command the skill runs next reports the problem
    with locked(paths["dir"]):
        data = load_tasks(paths)
        archive_data = load_archive(paths)
        added = phases_to_sprints(data, archive_data["tasks"], vocab, now_iso())
        if isinstance(archive_data.get("meta"), dict):
            archive_data["meta"].pop("phases", None)
        vocabulary(data)
        write_json_atomic(paths["tasks"], data)
        written = [paths["tasks"]]
        if os.path.isfile(archive):
            write_json_atomic(archive, archive_data)
            written.append(archive)
    if in_git(paths["root"]):
        git(["add", "--"] + [rel(paths["root"], p) for p in written], paths["root"])
    return added


WITH_USER_MIGRATION = "interactive-label"


def migrate_interactive(paths):
    """Tasks labelled `interactive` (work done with the user, T-240) get `with: user` (SR-6.5), once: the change is
    recorded in meta.migrations, so a `with` taken off later isn't put back. The label stays. Only unfinished tasks in
    tasks.json; nothing is written when none has the label. Returns how many tasks it changed."""
    def todo(data):
        meta = data.get("meta") if isinstance(data.get("meta"), dict) else {}
        if WITH_USER_MIGRATION in (meta.get("migrations") or []):
            return []
        vocab = vocabulary(data)
        return [
            t
            for t in data["tasks"]
            if isinstance(t, dict)
            and "interactive" in (t.get("labels") or [])
            and not t.get("with")
            and kind_of(vocab, t.get("status")) not in ("done", "skipped")
        ]

    try:
        if not todo(load_tasks(paths)):
            return 0
    except AtmError:
        return 0  # an unreadable file: the command the skill runs next reports the problem
    with locked(paths["dir"]):
        data = load_tasks(paths)
        tasks = todo(data)
        for task in tasks:
            task["with"] = "user"
        meta = data.setdefault("meta", {})
        meta["migrations"] = list(meta.get("migrations") or []) + [WITH_USER_MIGRATION]
        write_json_atomic(paths["tasks"], data)
    if in_git(paths["root"]):
        git(["add", "--", rel(paths["root"], paths["tasks"])], paths["root"])
    return len(tasks)


def ensure_gitignore(task_dir):
    """Add any missing runtime entries to the folder's .gitignore, keeping the user's lines. Returns True if changed."""
    p = os.path.join(task_dir, ".gitignore")
    try:
        with open(p, encoding="utf-8") as f:
            text = f.read()
    except OSError:
        text = ""
    lines = text.splitlines()
    missing = [entry for entry in IGNORED if entry not in lines]
    if not missing:
        return False
    if not text:
        text = "# atm runtime files (managed by atm)\n"
    elif not text.endswith("\n"):
        text += "\n"
    with open(p, "w", encoding="utf-8") as f:
        f.write(text + "".join(entry + "\n" for entry in missing))
    return True


def migrate(paths):
    """Move a legacy tasks.json into the task folder, set the folder up, and turn phases into sprints. Never commits."""
    root, task_dir, legacy = paths["root"], paths["dir"], paths["legacy"]
    result = {
        "moved": None,
        "removed_dashboard": None,
        "warning": None,
        "gitignore_updated": False,
        "merge_driver_updated": False,
        "cleaned_up": None,
        "finish_times_filled": 0,
        "sprints_from_phases": None,
        "with_user_from_label": 0,
    }
    if legacy and paths["tasks_exists"]:
        result["warning"] = (
            f"both {rel(root, legacy['tasks'])} and {rel(root, paths['tasks'])} exist; "
            f"using {rel(root, paths['tasks'])}. Remove the old file when you no longer need it."
        )
    elif legacy:
        os.makedirs(task_dir, exist_ok=True)
        tracked = is_tracked(root, legacy["tasks"])
        if tracked:
            if git(["mv", "--", rel(root, legacy["tasks"]), rel(root, paths["tasks"])], root) is None:
                raise AtmError(f"git mv {rel(root, legacy['tasks'])} failed")
        else:
            os.rename(legacy["tasks"], paths["tasks"])
        result["moved"] = {"from": rel(root, legacy["tasks"]), "to": rel(root, paths["tasks"]), "git": tracked}
        if legacy["dashboard"]:
            if is_tracked(root, legacy["dashboard"]):
                git(["rm", "--cached", "-q", "--", rel(root, legacy["dashboard"])], root)
            os.remove(legacy["dashboard"])
            result["removed_dashboard"] = rel(root, legacy["dashboard"])
    if not os.path.isfile(paths["tasks"]):
        return result
    if ensure_gitignore(task_dir):
        result["gitignore_updated"] = True
        if in_git(root):
            git(["add", "--", rel(root, os.path.join(task_dir, ".gitignore"))], root)
    result["merge_driver_updated"] = ensure_merge_driver(paths)
    if not os.path.exists(os.path.join(task_dir, CLAIMS_SNAPSHOT)):
        # So the dashboard finds the file from the start instead of logging a 404 on every refresh
        write_claims_snapshot(paths, read_store(paths)["claims"])
    result["cleaned_up"] = clean_up_old_dashboard(task_dir)
    result["finish_times_filled"] = backfill_finish_times(paths)
    result["sprints_from_phases"] = migrate_phases(paths)
    result["with_user_from_label"] = migrate_interactive(paths)
    try:
        hint = archive_hint(paths, load_tasks(paths))
    except AtmError:
        hint = None  # the command the skill runs next reports the problem
    if hint:
        result["suggest"] = {"archive": hint}
    return result


def describe_sprints_from_phases(sprints):
    """What the phase migration did, said once (it doesn't run again)."""
    if not sprints:
        return ["Moved the tasks' phases to their sprints (staged, not committed)."]
    closed = [s["id"] for s in sprints if s["state"] == "closed"]
    planned = [s["id"] for s in sprints if s["state"] == "planned"]
    count = "1 phase" if len(sprints) == 1 else f"{len(sprints)} phases"
    lines = [f"Phases are now sprints: turned {count} into sprints (staged, not committed)."]
    if closed:
        lines.append(f"Closed (all their tasks finished): {', '.join(closed)}.")
    if planned:
        lines.append(f"Planned: {', '.join(planned)}.")
        lines.append("None is active yet: plan one with /atm:plan-sprint.")
    return lines


def describe_migration(r, paths):
    lines = []
    if r["warning"]:
        lines.append("warning: " + r["warning"])
    if r["moved"]:
        line = f"Moved {r['moved']['from']} to {r['moved']['to']}"
        if r["moved"]["git"]:
            line += " (staged with git mv, not committed)"
        if r["removed_dashboard"]:
            line += f"; removed {r['removed_dashboard']} (the dashboard now comes with the plugin: /atm:serve)"
        lines.append(line + ".")
    elif not os.path.isfile(paths["tasks"]):
        lines.append("No tasks.json found. Run /atm:create-tasks to create one.")
    cleaned = r["cleaned_up"]
    if cleaned:
        line = "Removed this project's old dashboard files"
        if cleaned["server"]:
            line += f" and stopped its old dashboard server (pid {cleaned['server']})"
        lines.append(line + ": one dashboard for all your projects now comes with the plugin (/atm:serve).")
    filled = r.get("finish_times_filled")
    if filled:
        tasks = "1 finished task" if filled == 1 else f"{filled} finished tasks"
        lines.append(f"Filled in the finish time of {tasks} that had none, from the last update (completed_at/skipped_at).")
    sprints = r.get("sprints_from_phases")
    if sprints is not None:
        lines.extend(describe_sprints_from_phases(sprints))
    together = r.get("with_user_from_label")
    if together:
        tasks = "1 task" if together == 1 else f"{together} tasks"
        lines.append(
            f"Gave {tasks} labelled interactive `with: user` (done together with you; unattended loops skip them)."
        )
    if not lines:
        lines.append("Nothing to migrate.")
    if r.get("suggest"):
        lines.append(r["suggest"]["archive"]["text"])
    return lines


# ==== db/registry.py ====
# The user's project registry ($ATM_HOME/projects.json): the projects the dashboard server shows (the `Registry` of
# docs/layered-architecture.md §3.2).
#
# One file per user, guarded by $ATM_HOME/.lock (not by any project's lock, and never taken while holding one), written
# atomically. A linked git worktree registers its repository's main worktree. Format: docs/multi-project-api.md, 5.1.

REGISTRY_FILE = "projects.json"
REGISTRY_VERSION = 1
SEEN_EVERY = 24 * 3600  # seconds between seen_at updates, so the registry isn't rewritten on every command

# A project id is a key in URLs (#project=<id>, p/<id>/...): letters, digits, '.', '_' and '-'
PROJECT_ID = re.compile(r"[A-Za-z0-9._-]{1,64}")


def unique_id(registry, path, wanted):
    """The id for the project at `path`: `wanted` unless another project already has it or it isn't a valid id; then
    one made from the path, so two projects never shadow each other (F18)."""
    taken = {e.get("id") for e in registry["projects"] if e.get("path") != path}
    if PROJECT_ID.fullmatch(wanted) and wanted not in taken:
        return wanted
    suffix = hashlib.sha1(path.encode("utf-8")).hexdigest()[:7]
    base = wanted if PROJECT_ID.fullmatch(wanted) and len(wanted) <= 56 else "p"
    return f"{base}-{suffix}"


def project_meta(path):
    """The meta of the project at `path`, or None without a readable tasks.json."""
    data = read_json(os.path.join(path, TASK_DIR, TASKS_FILE))
    if not isinstance(data, dict) or not isinstance(data.get("tasks"), list):
        return None
    return data.get("meta") if isinstance(data.get("meta"), dict) else {}


def project_id(meta, path):
    """meta.id, else "p" and 7 hex characters of the path's SHA-1 (stable until meta.id is set)."""
    if isinstance(meta.get("id"), str) and meta["id"]:
        return meta["id"]
    return "p" + hashlib.sha1(path.encode("utf-8")).hexdigest()[:7]


def project_name(meta, path):
    return meta.get("project") if isinstance(meta.get("project"), str) and meta["project"] else os.path.basename(path)


def listed(meta):
    dashboard = meta.get("dashboard")
    return not (isinstance(dashboard, dict) and dashboard.get("listed") is False)


def is_missing(entry):
    return project_meta(entry["path"]) is None


def empty_registry():
    return {"version": REGISTRY_VERSION, "projects": []}


def read_registry(home):
    """(registry, damaged): an unreadable projects.json is moved aside to projects.json.damaged-<time> (its path is
    returned as `damaged`) and replaced by an empty registry. Call it holding the registry lock."""
    path = os.path.join(home, REGISTRY_FILE)
    if not os.path.exists(path):
        return empty_registry(), None
    data = read_json(path)
    entries = data.get("projects") if isinstance(data, dict) else None
    if isinstance(entries, list) and all(isinstance(e, dict) and isinstance(e.get("path"), str) for e in entries):
        return data, None
    damaged = f"{path}.damaged-{now().strftime('%Y%m%dT%H%M%SZ')}"
    os.replace(path, damaged)
    return empty_registry(), damaged


def write_registry(home, registry):
    write_json_atomic(os.path.join(home, REGISTRY_FILE), registry)


def find_entry(registry, path):
    return next((e for e in registry["projects"] if e["path"] == path), None)


def entry_for(registry, path, meta, stamp):
    return {
        "id": unique_id(registry, path, project_id(meta, path)),
        "name": project_name(meta, path),
        "path": path,
        "added_at": stamp,
        "seen_at": stamp,
    }


def seen_long_ago(entry):
    """True when the entry's seen_at is a day old (or unreadable): time to refresh it."""
    try:
        return (now() - parse_iso(entry.get("seen_at"))).total_seconds() >= SEEN_EVERY
    except (TypeError, ValueError):
        return True


def update_entry(registry, path, meta):
    """Add, refresh or (when the project opted out) remove the entry for `path`. Returns what happened: "added",
    "updated", "unchanged", "removed" or "unlisted"."""
    entry = find_entry(registry, path)
    if not listed(meta):
        if entry:
            registry["projects"].remove(entry)
            return "removed"
        return "unlisted"
    stamp = now_iso()
    if entry is None:
        registry["projects"].append(entry_for(registry, path, meta, stamp))
        return "added"
    fresh = {"id": unique_id(registry, path, project_id(meta, path)), "name": project_name(meta, path)}
    changed = any(entry.get(k) != v for k, v in fresh.items())
    if not (changed or seen_long_ago(entry)):
        return "unchanged"
    entry.update(fresh)
    entry["seen_at"] = stamp
    return "updated"


def register(paths, wait=True):
    """Register the project of `paths` (its main worktree). With wait=False the registry lock is tried once and the
    project is skipped if the lock is taken (the plugin's hook must never wait). Returns update_entry's result,
    "busy" when skipped, or None when the project has no tasks.json."""
    path = main_worktree(paths["root"])
    meta = project_meta(path)
    if meta is None:
        meta = project_meta(paths["root"])
    if meta is None:
        return None
    home = atm_home()
    ensure_home(home)
    try:
        with locked(home, timeout=None if wait else 0):
            registry, _ = read_registry(home)
            result = update_entry(registry, path, meta)
            if result not in ("unchanged", "unlisted"):
                write_registry(home, registry)
            return result
    except AtmError:
        if wait:
            raise
        return "busy"


def register_if_needed(paths):
    """The plugin hook's cheap path, run after every tool call: one read of projects.json, and registering (without
    waiting for the lock) only when the project isn't listed yet or was last seen a day ago."""
    path = main_worktree(paths["root"])
    registry = read_json(os.path.join(atm_home(), REGISTRY_FILE))
    entries = registry.get("projects") if isinstance(registry, dict) else None
    entry = next((e for e in entries or [] if isinstance(e, dict) and e.get("path") == path), None)
    if entry is not None and not seen_long_ago(entry):
        return "unchanged"
    return register(paths, wait=False)


def registered_id(paths):
    """(id, listed) of the project at `paths` as the registry has it: the id it gave the project (meta.id, or another
    when that one is taken, unique_id), and whether it is listed on the dashboard."""
    path = main_worktree(paths["root"])
    meta = project_meta(path) or project_meta(paths["root"]) or {}
    registry = read_json(os.path.join(atm_home(), REGISTRY_FILE)) or {}
    entry = next((e for e in registry.get("projects", []) if isinstance(e, dict) and e.get("path") == path), None)
    return (entry or {}).get("id") or project_id(meta, path), listed(meta)


def resolve_project(target):
    """The main worktree folder of the project at `target` (a path), and its meta; refuses a folder without tasks."""
    folder = os.path.abspath(os.path.expanduser(target or os.getcwd()))
    if not os.path.isdir(folder):
        raise AtmError(f"{target}: not a directory", 2)
    paths = find_paths(folder)
    path = main_worktree(paths["root"])
    meta = project_meta(path) or project_meta(paths["root"])
    if meta is None:
        raise AtmError(f"no tasks.json in {os.path.join(path, TASK_DIR)}. Run /atm:create-tasks there first.")
    return path, meta


class FileRegistry:
    """The registry in $ATM_HOME/projects.json. Each method holds the registry's lock while it reads and writes, and
    returns `damaged`: where an unreadable projects.json was moved (read_registry), or None."""

    def __init__(self, home=None):
        self.home = home or atm_home()

    def _locked(self):
        ensure_home(self.home)
        return locked(self.home)

    def file(self):
        """The registry's file: the server re-reads it when it changes."""
        return os.path.join(self.home, REGISTRY_FILE)

    def list_projects(self):
        """{projects: the entries, each with `missing` (no readable tasks.json there now), damaged}."""
        with self._locked():
            registry, damaged = read_registry(self.home)
            if damaged:
                write_registry(self.home, registry)
        return {"projects": [{**e, "missing": is_missing(e)} for e in registry["projects"]], "damaged": damaged}

    def add_project(self, target):
        """Register the project at `target` (a folder in it). {result: update_entry's, project: its entry, damaged}."""
        path, meta = resolve_project(target)
        if not listed(meta):
            raise AtmError(f"{path} opts out of the dashboard (meta.dashboard.listed is false)")
        with self._locked():
            registry, damaged = read_registry(self.home)
            result = update_entry(registry, path, meta)
            write_registry(self.home, registry)
        return {"result": result, "project": find_entry(registry, path), "damaged": damaged}

    def remove_project(self, target):
        """Unregister the project with the ID or path `target`. {removed: its entry, damaged}."""
        if not target:
            raise AtmError("remove needs a project ID or path (see `atm project list`)", 2)
        candidates = {target, os.path.abspath(os.path.expanduser(target))}
        if os.path.isdir(target):
            candidates.add(main_worktree(find_paths(os.path.abspath(target))["root"]))
        with self._locked():
            registry, damaged = read_registry(self.home)
            entry = next((e for e in registry["projects"] if e.get("id") in candidates or e["path"] in candidates), None)
            if entry:
                registry["projects"].remove(entry)
            if entry or damaged:
                write_registry(self.home, registry)
        if entry is None:
            raise AtmError(f"no registered project {target} (see `atm project list`)")
        return {"removed": entry, "damaged": damaged}

    def prune(self):
        """Unregister the projects whose task folder is gone. {removed: their entries, damaged}."""
        with self._locked():
            registry, damaged = read_registry(self.home)
            gone = [e for e in registry["projects"] if is_missing(e)]
            registry["projects"] = [e for e in registry["projects"] if e not in gone]
            if gone or damaged:
                write_registry(self.home, registry)
        return {"removed": gone, "damaged": damaged}

    def stamp(self):
        """The file's size and mtime (None while there is none): it changes when the list does."""
        try:
            st = os.stat(self.file())
        except OSError:
            return None
        return (st.st_size, st.st_mtime_ns)

    def entries(self):
        """The entries as the file has them now, without the lock (the server reads them on every request)."""
        data = read_json(self.file())
        entries = data.get("projects") if isinstance(data, dict) else None
        return [e for e in entries or [] if isinstance(e, dict) and isinstance(e.get("path"), str)]


# ==== db/working.py ====
# A store's working copy for one change (docs/layered-architecture.md §3.3): what atm-lib reads and changes inside a
# store's `change(fn)`, and the record of what it changed.
#
# A store runs `fn(work)` under its own isolation (the JSON-in-git store: its lock), writes what changed and only that,
# and raises what `fn` deferred after writing. `changes()` lists each task and claim `fn` changed, before and after: a
# store that keeps entities apart (a database) can turn the list into conditional writes, without atm-lib's rules
# changing. Until the last of the code from before the layers has moved, the copy is the whole task document.

def _stamp(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False)


class Working:
    def __init__(self, document, claims_store, here, size, before, archive=None):
        """`here` and `size` may be callables (worked out when asked); `before()` gives the document and the claims
        store as they were, from wherever the store keeps them (the JSON-in-git store: its files, not written until
        the change ends), so recording what changed costs nothing unless `changes()` is asked for. `archive()` loads
        the archive document, only when a change asks for it."""
        self.document = document
        self.claims_store = claims_store
        self._here = here
        self._size = size
        self._before = before
        self._load_archive = archive
        self._archive = None
        self._before_archive = None
        self.archive_first = False
        self._deferred = None
        self._before_store = _stamp(claims_store)

    def archive(self):
        """The archive document ({"meta", "tasks"}), to read and change in place; loaded on first use."""
        if self._archive is None:
            self._archive = self._load_archive() if self._load_archive else {"meta": {}, "tasks": []}
            self._before_archive = _stamp(self._archive)
        return self._archive

    def moving_into_archive(self):
        """This change moves tasks from the working tasks into the archive: a store that writes the two apart writes
        the archive first, so a crash between leaves a task in both, never in neither (DM-5.2). Without it (a
        restore), the working tasks go first."""
        self.archive_first = True

    def archive_changed(self):
        """Whether the archive was asked for and changed: the store writes it then, and only then."""
        return self._archive is not None and _stamp(self._archive) != self._before_archive

    @staticmethod
    def _entities(document, claims_store):
        tasks = {t.get("id"): t for t in document["tasks"] if isinstance(t, dict)}
        return {"task": tasks, "claim": dict(claims_store.get("claims") or {})}

    def tasks(self):
        """The working tasks, to read and change in place."""
        return self.document["tasks"]

    def claims(self):
        """The claims store: `claims`, `completed`, `waiting`, `resolved`, `hints`, to read and change in place."""
        return self.claims_store

    def here(self):
        """The working copy this change runs in: `worktree` (its folder, None for a store without them) and `branch`."""
        if callable(self._here):
            self._here = self._here()
        return self._here

    def size(self):
        """How big the task data is, in bytes, as it was read for this change (the archive hint)."""
        if callable(self._size):
            self._size = self._size()
        return self._size

    def defer(self, error):
        """Raise `error` once the change is written: a lease that expired is written as expired, then reported."""
        self._deferred = error

    def raise_deferred(self):
        if self._deferred is not None:
            raise self._deferred

    def changes(self):
        """What changed: [{"entity": "task" | "claim", "id", "before", "after"}], `before` or `after` None for an entity
        created or removed. The rest of the claims store (completed, waiting, resolved, hints) is in `store_changed`."""
        before = self._entities(*self._before())
        after = self._entities(self.document, self.claims_store)
        found = []
        for entity in ("task", "claim"):
            old, new = before[entity], after[entity]
            for key in list(old) + [k for k in new if k not in old]:
                if old.get(key) != new.get(key):
                    found.append({"entity": entity, "id": key, "before": old.get(key), "after": new.get(key)})
        return found

    def store_changed(self):
        return _stamp(self.claims_store) != self._before_store


# ==== db/jsongit/store.py ====
# The JSON-in-git store (docs/layered-architecture.md §3): the project's task files, read and written as atm always
# has: tasks.json and the archive (files.py), the claims store shared by a clone's worktrees (claims.py), under the
# task folder's lock (lock.py); its worktrees' copies (worktrees.py) and the project's logo (logo.py).

def file_stamp(p):
    """(size, nanosecond mtime) of a file, or None if it doesn't exist."""
    try:
        st = os.stat(p)
    except OSError:
        return None
    return (st.st_size, st.st_mtime_ns)


class JsonGitStore:
    def __init__(self, paths, tolerant=False):
        """`tolerant`: read a malformed task file as far as it can be read, and say what is wrong (`problems()`),
        instead of refusing it: for showing it (the REST API's reads, T-377). Writes are strict either way."""
        self.paths = paths
        self.tolerant = tolerant
        self._data = None
        self._archived = None  # the lookup of this project's archive, made on first use
        self._problems = {"file": [], "tasks": {}}

    def _loaded(self):
        # Read once per store: one atm command works on one snapshot of the file
        if self._data is None:
            if self.tolerant:
                self._data, self._problems = load_tasks_tolerant(self.paths)
            else:
                self._data = load_tasks(self.paths)
        return self._data

    def problems(self):
        """What a tolerant read found wrong: {"file": [...], "tasks": {id: [...]}}; nothing for a strict one."""
        self._loaded()
        return self._problems

    def get_vocabularies(self):
        return vocabulary(self._loaded())

    def get_settings(self):
        return settings_of(self._loaded().get("meta"))

    def list_sprints(self):
        return self.get_vocabularies()["sprints"]

    def list_releases(self):
        return self.get_vocabularies()["releases"]

    def list_components(self):
        return self.get_vocabularies()["components"]

    def list_tasks(self):
        return self._loaded()["tasks"]

    def get_task(self, task_id):
        for task in self._loaded()["tasks"]:
            if task.get("id") == task_id:
                return task
        return None

    def get_archived_task(self, task_id):
        """This project's archived task with this ID, or None; its own archive, read once per store."""
        if self._archived is None:
            self._archived = archive_of(self.paths)
        return self._archived(task_id)

    def list_archived(self):
        return [t for t in load_archive(self.paths)["tasks"] if isinstance(t, dict)]

    def read_claims(self):
        return read_store(self.paths)

    def document(self):
        return self._loaded()

    def sizes(self):
        archive = archive_path(self.paths)
        return {
            "tasks": os.path.getsize(self.paths["tasks"]),
            "archive": os.path.getsize(archive) if os.path.isfile(archive) else 0,
        }

    def working_copy(self):
        return self.paths["root"]

    def files_for_commit(self):
        """tasks.json travels with the code: `atm commit` commits it with the task's files (relative to the root)."""
        return [os.path.relpath(self.paths["tasks"], self.paths["root"])]

    def exists(self):
        return os.path.isfile(self.paths["tasks"])

    def create(self, document):
        """Write the project's first task data (`atm init`); Conflict if it has some already."""
        folder = self.paths["dir"]
        with locked(folder) if os.path.isdir(folder) else contextlib.nullcontext():
            if self.exists():
                raise Conflict(f"{self.paths['tasks']} exists already")
            os.makedirs(folder, exist_ok=True)
            write_json_atomic(self.paths["tasks"], document)
        self._data = None

    def stamp(self):
        """What changes when the data does, for the REST API's ETags: per part ({tasks, archive, claims, head, copies}),
        a value that changes with it (here, each file's size and mtime; `head` the checked-out branch's; `copies` the
        worktrees and their task files'), None for a part that isn't there."""
        relative = os.path.relpath(self.paths["tasks"], self.paths["root"])
        return {
            "tasks": file_stamp(self.paths["tasks"]),
            "archive": file_stamp(archive_path(self.paths)),
            "claims": file_stamp(claims_store_path(self.paths)),
            "head": file_stamp(os.path.join(self.paths["root"], ".git", "HEAD")),
            "copies": [(t, file_stamp(os.path.join(t["path"], relative))) for t in worktrees(self.paths["root"])],
        }

    def logo(self, meta):
        """The project's logo, a file in the working copy: {path (relative), file, type, source: "set" (meta.logo) or
        "found" (in the usual places)}, or None."""
        found = find_logo(self.paths["root"], meta)
        return {"path": found[0], "file": found[1], "type": found[2], "source": found[3]} if found else None

    def check_logo(self, relative):
        """Refuse (AtmError, exit code 2) a logo the dashboard can't be given: not an image atm serves, outside the
        project, missing or too large."""
        logo_file(self.paths["root"], relative)

    def working_copies(self):
        """The project's working copies, main first: its git worktrees, [{path, branch, main}]; [] outside git."""
        return worktrees(self.paths["root"])

    def copy_tasks(self, folder):
        """The tasks in the working copy at `folder` as its tasks.json has them (entries that aren't tasks left out),
        for showing beside the main one's; [] when it has none."""
        data = read_json(os.path.join(folder, os.path.relpath(self.paths["tasks"], self.paths["root"])))
        tasks = data.get("tasks") if isinstance(data, dict) else None
        return [t for t in tasks or [] if isinstance(t, dict) and isinstance(t.get("id"), str)]

    def copy_store(self, folder):
        """The working copy at `folder` as a store of its own (its tasks.json and archive), read tolerantly, for its
        tasks' derived values. Its task folder's symlink checks run as for any project (docs/security-review.md F8)."""
        return JsonGitStore(find_paths(folder), tolerant=True)

    def raw(self):
        """The task data as stored, unchecked, for `atm check` to report on: {"document": the file's JSON (None when
        it isn't a task file), "archive": the archived tasks, "archive_error": why the archive can't be read}."""
        if not os.path.isfile(self.paths["tasks"]):
            raise AtmError(f"no tasks.json in {self.paths['dir']}. Run /atm:create-tasks first.")
        data = read_json(self.paths["tasks"])
        if not isinstance(data, dict) or not isinstance(data.get("tasks"), list):
            data = None
        try:
            archive, error = load_archive(self.paths)["tasks"], None
        except AtmError as e:
            archive, error = [], str(e)
        return {"document": data, "archive": archive, "archive_error": error}

    def change(self, fn):
        """Run `fn(work)` under the task folder's lock (and the claims store's), as every atm write always has: read
        tasks.json and the claims store, let `fn` change them, write what changed. If `fn` raises, nothing is written.
        (docs/layered-architecture.md §3.3: the transitional form of `apply`.)"""
        root, tasks_file = self.paths["root"], self.paths["tasks"]
        with locked_with_claims(self.paths):
            work = Working(
                load_tasks(self.paths),
                read_store(self.paths),
                # The branch as everywhere else (§11.8): from HEAD, a detached one as its short SHA
                lambda: {"worktree": root, "branch": checked_out_branch(root)},
                lambda: os.path.getsize(tasks_file),
                # The files still hold what was read: nothing is written before fn returns
                lambda: (read_json(tasks_file), read_store(self.paths)),
                lambda: load_archive(self.paths),
            )
            result = fn(work)

            def write_tasks():
                # Written only when its text changed: a read-only change (atm next, a heartbeat) leaves the file and
                # its mtime alone, so open dashboards don't reload for nothing
                text = to_json(work.document) + "\n"
                with open(tasks_file, encoding="utf-8") as f:
                    stored = f.read()
                # The same data in another layout (a file edited by hand) is unchanged too
                if stored != text and json.loads(stored) != work.document:
                    write_text_atomic(tasks_file, text)

            def write_archive():
                if work.archive_changed():
                    write_json_atomic(archive_path(self.paths), work.archive())
                    if work.archive_first:
                        ensure_archive_merge(self.paths)  # tasks moved in: the archive merges task by task too

            # A move into the archive writes it first; anything else writes tasks.json first (DM-5.2)
            for write in (write_archive, write_tasks) if work.archive_first else (write_tasks, write_archive):
                write()
            if work.store_changed():
                write_store(self.paths, work.claims_store)
        self._data = self._archived = None  # this store's snapshot is out of date now
        work.raise_deferred()
        return result


# ==== db/memory.py ====
# The in-memory store: a project's data in plain lists, for atm-lib's tests. It also shows that atm-lib depends on
# the store's interface only: the contract tests (tests/lib/) run the same checks on it and on the JSON-in-git store.

def empty_claims():
    return {"claims": {}, "completed": {}, "waiting": {}, "resolved": {}, "hints": {}}


class MemoryStore:
    def __init__(self, tasks=None, archive=None, meta=None, claims=None, problems=None, created=True):
        """`problems`: what a tolerant read would have found ({"file": [...], "tasks": {id: [...]}}), for the tests of
        what atm-lib makes of them; the memory holds data, not a file to read. `created` false: a project with no
        task data yet (`atm init` creates it)."""
        self.created = created
        self.version = 0  # counts the changes: the stamp
        self.meta = dict(meta or {})
        self.tasks = list(tasks or [])
        self.archive = list(archive or [])
        self.claims = {**empty_claims(), **(claims or {})}
        self._problems = {"file": [], "tasks": {}, **(problems or {})}

    def problems(self):
        return self._problems

    def get_vocabularies(self):
        return vocabulary(self.document())

    def get_settings(self):
        return settings_of(self.meta)

    def list_sprints(self):
        return self.get_vocabularies()["sprints"]

    def list_releases(self):
        return self.get_vocabularies()["releases"]

    def list_components(self):
        return self.get_vocabularies()["components"]

    def list_tasks(self):
        return self.tasks

    def get_task(self, task_id):
        return next((t for t in self.tasks if t.get("id") == task_id), None)

    def get_archived_task(self, task_id):
        return next((t for t in self.archive if t.get("id") == task_id), None)

    def list_archived(self):
        return self.archive

    def read_claims(self):
        return self.claims

    def document(self):
        return {"meta": self.meta, "tasks": self.tasks}

    def sizes(self):
        # What the task files would be: the same JSON, indented as atm writes it
        size = lambda value: len(json.dumps(value, indent=2, ensure_ascii=False).encode("utf-8"))
        return {"tasks": size(self.document()), "archive": size({"meta": {}, "tasks": self.archive}) if self.archive else 0}

    def working_copy(self):
        return None

    def files_for_commit(self):
        return []  # nothing of the memory travels with code commits

    def exists(self):
        return self.created

    def create(self, document):
        if self.created:
            raise Conflict("the project has task data already")
        self.meta, self.tasks, self.created = dict(document.get("meta") or {}), list(document["tasks"]), True
        self.version += 1

    def stamp(self):
        return {"tasks": self.version, "archive": self.version, "claims": self.version, "head": None, "copies": []}

    def logo(self, meta):
        return None  # no files: a logo would be uploaded to a store like this one

    def check_logo(self, relative):
        raise Invalid(f"{relative}: this project keeps no files for a logo")

    def working_copies(self):
        return []

    def copy_tasks(self, folder):
        return []

    def copy_store(self, folder):
        raise NotFound(f"{folder}: an in-memory project has no working copies")

    def raw(self):
        return {"document": self.document(), "archive": self.archive, "archive_error": None}

    def change(self, fn):
        """Run `fn(work)` on copies, and keep them only when it succeeds: a failed change leaves the store as it was."""
        original, original_claims = self.document(), self.claims
        document, claims = copy.deepcopy(original), copy.deepcopy(original_claims)
        work = Working(
            document,
            claims,
            {"worktree": None, "branch": None},
            lambda: self.sizes()["tasks"],
            lambda: (original, original_claims),  # replaced, not changed, when the change is kept
            lambda: {"meta": {}, "tasks": copy.deepcopy(self.archive)},
        )
        result = fn(work)
        self.meta, self.tasks, self.claims = document["meta"], document["tasks"], claims
        if work.archive_changed():
            self.archive = work.archive()["tasks"]
        self.changes = work.changes()  # what the last change did, for the tests
        self.version += 1
        work.raise_deferred()
        return result


# ==== vcs/git.py ====
# Git as the code's version control (docs/layered-architecture.md §2): committing exactly the files named, the
# history, the base branch, the remote. Git's other role, carrying the task files, is the JSON-in-git store's.

# Pathspecs that stage more than the files named: another agent's work can be in the same tree
WHOLE_TREE = {".", "-A", "--all", "-u", "--update", ":/", "*"}
# git reads a path as a pattern (`*.env` is every .env file, `src/[id]` matches `src/i`); with this, a path names
# exactly one file (docs/security-review.md F6)
LITERAL = ["--literal-pathspecs"]


class GitVcs:
    def __init__(self, root):
        self.root = root

    def in_repository(self):
        return in_git(self.root)

    def branch(self):
        """The checked-out branch (a detached HEAD's short SHA), read from HEAD without running git, as the store's
        claims record it (host/git.py); None outside git."""
        return checked_out_branch(self.root)

    def user_name(self):
        """Who git says the user is (`user.name`), for naming the actor; None when it isn't set."""
        return git(["config", "user.name"], self.root)

    def default_base(self):
        """The remote's default branch (origin/HEAD), else main."""
        ref = git(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"], self.root)
        return ref.split("/", 1)[1] if ref and "/" in ref else "main"

    def remote_url(self):
        """The origin remote's address, as git has it (atm repository turns it into a web address)."""
        return git(["remote", "get-url", "origin"], self.root)

    def paths(self, files, cwd=None):
        """The files to commit, relative to the repository, from names relative to `cwd` (None: the repository);
        refuses anything that would stage more than the files named, and a missing file git doesn't track."""
        picked = []
        for name in files:
            if name in WHOLE_TREE or name.startswith(":"):
                raise Invalid(f"atm vcs commit stages only the files you name; {name!r} would stage the whole tree")
            full = os.path.realpath(os.path.join(cwd or self.root, name))
            relative = os.path.relpath(full, self.root)
            if relative == "." or relative.startswith(".." + os.sep) or relative == "..":
                raise Invalid(f"{name} is outside the repository")
            if os.path.isdir(full):
                raise Invalid(f"{name} is a directory: name the files in it")
            # A deleted file is fine as long as git knows it: still in the index (deleted by hand) or in HEAD (`git rm`)
            if (
                not os.path.exists(full)
                and git(LITERAL + ["ls-files", "--error-unmatch", "--", relative], self.root) is None
                and git(["cat-file", "-e", f"HEAD:{relative}"], self.root) is None
            ):
                raise NotFound(f"{name}: no such file, and git doesn't track it")
            if relative not in picked:
                picked.append(relative)
        return picked

    def changes_under(self, folders, cwd=None):
        """`atm commit --dir` (T-405): every change git sees under each folder (named relative to `cwd`, None: the
        repository), as `git status` lists it: modified, added, deleted, renamed (both names) and new files git doesn't
        ignore; relative to the repository, in order. Refuses the repository's root or above, a folder outside it, the
        task folder (tasks.json goes with every commit anyway), a file, and a folder with nothing changed."""
        picked = []
        for name in folders:
            full = os.path.realpath(os.path.join(cwd or self.root, name))
            relative = os.path.relpath(full, self.root).replace(os.sep, "/")
            if relative in (".", "..") or relative.startswith("../"):
                raise Invalid(f"--dir {name}: name a folder inside the repository, not the repository itself or above it")
            if relative == ".agentic-task-manager" or relative.startswith(".agentic-task-manager/"):
                raise Invalid(f"--dir {name}: the task folder isn't committed this way (tasks.json goes with every commit)")
            if os.path.isfile(full):
                raise Invalid(f"--dir {name} is a file: name it with --file")
            status = run_git(
                LITERAL + ["status", "--porcelain", "-z", "--untracked-files=all", "--", relative], self.root, strip=False
            )
            entries = [e for e in status.split("\0") if e]
            found = []
            i = 0
            while i < len(entries):
                entry = entries[i]
                i += 1
                if len(entry) < 4:
                    continue
                code, path = entry[:2], entry[3:]
                found.append(path)
                # A rename or a copy: the next entry is the name it had, which goes in the commit too
                if "R" in code or "C" in code:
                    if i < len(entries):
                        found.append(entries[i])
                    i += 1
            found = [p for p in found if p == relative or p.startswith(relative + "/")]
            if not found:
                raise NotFound(f"--dir {name}: nothing changed under it")
            picked += [p for p in found if p not in picked]
        return picked

    def commit(self, files, message):
        """Commit exactly `files` (relative to the repository); whatever else is staged stays staged and out of this
        commit. Returns the new commit's SHA."""
        # A deletion already staged with `git rm` is in neither the tree nor the index: there is nothing to add for it
        to_add = [
            f
            for f in files
            if git(LITERAL + ["ls-files", "--error-unmatch", "--", f], self.root) is not None
            or os.path.lexists(os.path.join(self.root, f))
        ]
        if to_add:
            run_git(LITERAL + ["add", "-A", "--"] + to_add, self.root)
        run_git(LITERAL + ["commit", "-q", "-m", message, "--"] + list(files), self.root)
        return run_git(["rev-parse", "HEAD"], self.root)

    def resolve(self, rev):
        """A revision's SHA, or None if there is no such commit."""
        return git(["rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}"], self.root)

    def in_history(self, sha):
        """Whether the commit is an ancestor of HEAD (on the current branch)."""
        return git(["merge-base", "--is-ancestor", sha, "HEAD"], self.root) is not None

    def message(self, sha):
        return run_git(["log", "-1", "--format=%B", sha], self.root)

    # The working copy's files, for the document viewer (docs/compmd-viewer-design.md §4). atm-lib checks the path
    # first (lib/documents.py); these read only below the root, through no symbolic link

    def _file(self, relative):
        """The file's absolute path, or None when a segment from the root down is a symbolic link or it isn't a
        regular file."""
        current = self.root
        for segment in relative.split("/"):
            current = os.path.join(current, segment)
            if os.path.islink(current):
                return None
        return current if os.path.isfile(current) else None

    def file_stamp(self, relative):
        path = self._file(relative)
        if path is None:
            return None
        try:
            st = os.stat(path)
        except OSError:
            return None
        return (st.st_size, st.st_mtime_ns)

    def read_file(self, relative):
        path = self._file(relative)
        if path is None:
            raise NotFound(f"{relative}: no such file")
        with open(path, "rb") as f:
            return f.read()

    def _submodule(self, relative):
        """The submodule `relative` is in, nearest the root, as (its folder, the path inside it), or None. A submodule
        is a folder this repository records as a commit (mode 160000) with a checkout of its own (`.git`); git answers
        nothing about the files in it from here, only from its own repository (T-408). A nested repository that isn't a
        submodule is not one: its files stay this repository's to judge (and git refuses them, so they stay hidden)."""
        parts = relative.split("/")
        for i in range(1, len(parts)):
            folder = "/".join(parts[:i])
            if not os.path.lexists(os.path.join(self.root, folder, ".git")):
                continue
            for line in (git(LITERAL + ["ls-files", "--stage", "--", folder], self.root) or "").splitlines():
                meta, _, path = line.partition("\t")
                if path == folder and meta.split(" ", 1)[0] == "160000":
                    return folder, "/".join(parts[i:])
        return None

    def _in_submodule(self, relative):
        """The submodule's own GitVcs and the path inside it, or None outside one"""
        found = self._submodule(relative)
        return (GitVcs(os.path.join(self.root, found[0])), found[1]) if found else None

    def ignored(self, relative):
        """git's ignore rules, as `git status` applies them: a file git tracks is never ignored. check-ignore takes
        paths, not patterns (and refuses --literal-pathspecs). It fails closed: only git's "not ignored" (exit 1)
        lets a file through; an error counts as ignored. A file in a submodule: the submodule's own rules decide (git
        refuses the question from here), down to the innermost submodule."""
        if not self.in_repository():
            return False
        inside = self._in_submodule(relative)
        if inside:
            return inside[0].ignored(inside[1])
        return git_exit(["check-ignore", "-q", "--", relative], self.root) != 1

    def tracked(self, relative):
        if not self.in_repository():
            return None
        inside = self._in_submodule(relative)
        if inside:
            return inside[0].tracked(inside[1])
        return git(LITERAL + ["ls-files", "--error-unmatch", "--", relative], self.root) is not None

    def head(self):
        return self.resolve("HEAD") if self.in_repository() else None

    def submodule_of(self, relative):
        """For a file in a submodule (the innermost): {path: its folder from the root, commit: its HEAD, repository:
        its origin's web address or None}; None for any other file."""
        if not self.in_repository():
            return None
        found = self._submodule(relative)
        if not found:
            return None
        sub = GitVcs(os.path.join(self.root, found[0]))
        nested = sub.submodule_of(found[1])
        if nested:
            return {**nested, "path": f"{found[0]}/{nested['path']}"}
        return {"path": found[0], "commit": sub.head(), "repository": repository_url(sub.remote_url())}


# ==== lib/project.py ====
# A project as atm-lib sees it: where its data is kept, its code's version control, and who acts.

class Project:
    def __init__(self, store, vcs=None, actor=None):
        self.store = store
        self.vcs = vcs
        self.actor = actor


# ==== lib/state.py ====
# Task state machine using the transitions library.
#
# Declares the task status transitions as data, with their guards (conditions).
# Replaces the hand-rolled change_status() that was in common/model.py, and
# refuses and records exactly as it did: the same messages, exit codes, notes
# and key order in tasks.json (the scenarios in tests/scripts/ check them).
#
# The machine state is always a *kind* (todo, active, review, blocked, done,
# skipped), never a specific status name. The task's `status` field holds the
# project-specific status name (e.g. `in_progress` for kind `active`), mapped
# from the kind with status_for().
#
# Order of checks (as before): the resolution, then whether the command applies
# to the task's kind, then the guards, then the notes and the block's arguments.
# Only then does anything change, in one place (_apply).

class _TaskStateModel:
    """The machine's model: its `state` is the task's kind, starting from the task's status."""

    def __init__(self, kind):
        self.state = kind


class TaskMachine:
    """The status commands as transitions between the six kinds, with their guards.

    A guard raises AtmError with the message the CLI prints; transitions passes it through unchanged.
    """

    states = ["todo", "active", "review", "blocked", "done", "skipped"]

    def __init__(self, task, data, agent, archived, stamp):
        self.task = task
        self.data = data
        self.agent = agent
        self.archived = archived
        self.stamp = stamp
        self.vocab = vocabulary(data)
        self.current_status = task.get("status")
        self.current_kind = kind_of(self.vocab, self.current_status) or "todo"
        self._model = _TaskStateModel(self.current_kind)

        # Every transition checks the command's arguments last, after its own guards, as change_status did
        args = [self.arguments_valid]
        transitions = [
            # From review only once the review asked for rework or approved it (SR-3.9)
            {
                "trigger": "start",
                "source": ["todo", "review"],
                "dest": "active",
                "conditions": [self.review_decided, self.dependencies_done, *args],
            },
            {"trigger": "review", "source": "active", "dest": "review", "conditions": args},
            {
                "trigger": "done",
                "source": ["active", "review"],
                "dest": "done",
                "conditions": [self.review_mode_allows_done, *args],
            },
            {"trigger": "fail", "source": "active", "dest": "todo", "conditions": args},
            {"trigger": "release", "source": "active", "dest": "todo", "conditions": args},
            {"trigger": "reject", "source": "review", "dest": "todo", "conditions": args},
            # The review's outcome; the task stays in review until an agent takes it back (SR-6.1)
            {"trigger": "approve", "source": "review", "dest": "review", "conditions": args},
            {"trigger": "request_changes", "source": "review", "dest": "review", "conditions": args},
            {"trigger": "block", "source": ["todo", "active"], "dest": "blocked", "conditions": args},
            {"trigger": "unblock", "source": "blocked", "dest": "todo", "conditions": args},
            # Undo a done (or skipped) by mistake, or work that turns out unfinished
            {"trigger": "reopen", "source": ["done", "skipped"], "dest": "todo", "conditions": args},
            # A task that turns out not to be needed; reopen undoes it
            {"trigger": "skip", "source": ["todo", "blocked"], "dest": "skipped", "conditions": args},
        ]

        self.machine = Machine(
            model=self._model,
            states=TaskMachine.states,
            transitions=transitions,
            initial=self.current_kind,
            send_event=True,
            auto_transitions=False,
        )

    # ------------------------------------------------------------------
    # Guards
    # ------------------------------------------------------------------

    def review_decided(self, event):
        """start from review: only once the review asked for changes or approved it."""
        if self.current_kind == "review" and self.task.get("review_state") not in REWORK_STATES:
            raise AtmError(
                f"can't start {self.task['id']}: it waits for its review (atm task review approve or request-changes {self.task['id']} decides it)"
            )
        return True

    def dependencies_done(self, event):
        """start: every dependency is done (one only in the archive is)."""
        blocking = unfinished_dependencies(self.data, self.task, self.archived)
        if blocking:
            raise AtmError(
                f"can't start {self.task['id']}: waiting on " + ", ".join(f"{d} ({s})" for d, s in blocking)
            )
        return True

    def review_mode_allows_done(self, event):
        """done: an agent finishing a `blocker` or `async` task asks for the review instead (SR-3.8)."""
        check_review_mode(self.task, self.data, self.agent)
        return True

    def arguments_valid(self, event):
        """Every command: notes, reasons, questions and answers fit in a note; block says what it waits for."""
        kwargs = event.kwargs
        note, reason = kwargs.get("note"), kwargs.get("reason")
        if note is not None:
            check_note(note)
        if reason is not None:
            check_note(reason)
        for question in kwargs.get("questions") or ():
            check_note(question)
        if event.event.name == "block":
            if kwargs.get("waiting_on") not in WAITING_ON:
                raise AtmError(f"--on must be {' or '.join(WAITING_ON)}", 2)
            if not kwargs.get("questions") and not reason:
                raise AtmError("say what it waits for: --question (repeatable) or --reason", 2)
        for answer in kwargs.get("answers") or ():
            check_note(answer)
        return True

    # ------------------------------------------------------------------
    # Public entry point
    # ------------------------------------------------------------------

    def transition(self, command, **kwargs):
        """Run `command` on the task. Returns its status before the command; raises AtmError if it isn't allowed."""
        trigger = command.replace("-", "_")
        if command in ("done", "skip"):
            kwargs["resolution"] = self._check_resolution(command, kwargs)
        if not self.machine.get_transitions(trigger, source=self.current_kind):
            allowed = sorted(
                {t.source for t in self.machine.get_transitions(trigger)}, key=TaskMachine.states.index
            )
            expected = [s for kind in allowed for s in statuses_of(self.vocab, kind)]
            raise AtmError(
                f"can't {command} {self.task['id']}: it is {self.current_status}, not {' or '.join(expected)}"
            )
        # The guards raise with their message; one that returns False would leave the task unchanged
        if not getattr(self._model, trigger)(**kwargs):
            raise AtmError(f"can't {command} {self.task['id']}")
        self._apply(command, self._model.state, kwargs)
        return self.current_status

    def _check_resolution(self, command, kwargs):
        """done and skip record a resolution (SR-2.8); skip may name the task it duplicates."""
        resolution = kwargs.get("resolution") or ("done" if command == "done" else "wont_do")
        allowed = RESOLUTIONS if command == "done" else SKIP_RESOLUTIONS
        if resolution not in allowed:
            raise AtmError(f"--resolution must be one of {', '.join(allowed)}", 2)
        duplicate_of = kwargs.get("duplicate_of")
        if duplicate_of is not None and resolution != "duplicate":
            raise AtmError("--duplicate-of goes with --resolution duplicate", 2)
        if duplicate_of == self.task["id"]:
            raise AtmError(f"{self.task['id']} can't be a duplicate of itself", 2)
        return resolution

    # ------------------------------------------------------------------
    # Effects, once the transition is allowed
    # ------------------------------------------------------------------

    def _apply(self, command, new_kind, kwargs):
        """Record the command on the task, in the order change_status did (it shapes the key order in tasks.json)."""
        task, vocab, agent, stamp = self.task, self.vocab, self.agent, self.stamp
        note, reason = kwargs.get("note"), kwargs.get("reason")
        old_kind = self.current_kind

        # A block's or a review's start goes when the task moves on: keep each interval, so waiting time can be measured
        if old_kind == "blocked" and task.get("blocked_at"):
            add_interval(task, "blocks", task["blocked_at"], stamp, on=task.get("waiting_on"))
        elif old_kind == "review" and new_kind != "review" and task.get("review_requested_at"):
            # The round ends when the reviewer decided, with its outcome (SR-3.9)
            outcome = task.get("review_state") if task.get("review_state") in REWORK_STATES else None
            add_interval(task, "reviews", task["review_requested_at"], task.get("reviewed_at") or stamp, outcome=outcome)
        if new_kind != "review":
            task.pop("review_state", None)
            task.pop("reviewed_at", None)

        if new_kind == "todo":
            # Reopening, rejecting or failing a task undoes its work; releasing or unblocking it doesn't
            back_to_todo(task, vocab, agent, stamp, keep_commit=command not in ("reopen", "reject", "fail"))
            if kwargs.get("answered"):
                add_note(task, "answer", kwargs["answered"], agent, stamp, checked=False)
            if command == "fail":
                add_note(task, "failure", reason or "verification failed", agent, stamp)
            elif command == "reject":
                add_note(task, "rejection", reason, agent, stamp)
            elif command == "reopen":
                add_note(task, "note", f"Reopened: {reason}", agent, stamp)
            elif note:
                add_note(task, "handoff", note, agent, stamp)
            return

        if command in ("approve", "request-changes") and kwargs.get("status") is None:
            task["status"] = self.current_status  # it stays in its review status (a project may have several)
        else:
            task["status"] = status_for(vocab, new_kind, kwargs.get("status"))
        touch(task, agent, stamp)

        if command == "block":
            # Who started it stays visible while it waits; unblock returns it to to-do and clears the attempt
            task["waiting_on"] = kwargs.get("waiting_on")
            task["questions"] = list(kwargs.get("questions") or ())
            task["blocked_at"] = stamp
            if reason:
                add_note(task, "note", reason, agent, stamp)
        elif command == "review":
            # The work ends here: the review isn't agent work (DM-4.1); done keeps this timestamp
            task["review_requested_at"] = stamp
            task["review_state"] = "waiting"
            if note:
                add_note(task, "review", note, agent, stamp)
        elif command in ("approve", "request-changes"):
            # The task stays in review until an agent takes it back (`atm next` offers it first, SR-3.9)
            task["review_state"] = "approved" if command == "approve" else "changes_requested"
            task["reviewed_at"] = stamp
            if command == "request-changes":
                add_note(task, "rejection", reason, agent, stamp)
            else:
                add_note(task, "note", f"Approved: {note}" if note else "Approved", agent, stamp)
        elif command == "skip":
            for field in ("waiting_on", "questions", "blocked_at"):
                task.pop(field, None)
            task["skipped_at"] = stamp
            if kwargs["resolution"] != "wont_do":
                task["resolution"] = kwargs["resolution"]  # none means the default, won't do
            if kwargs.get("duplicate_of"):
                task["duplicate_of"] = kwargs["duplicate_of"]
            add_note(task, "note", f"Skipped: {reason}", agent, stamp)
        elif command == "start":
            task["started_at"] = stamp
            task.pop("review_requested_at", None)  # taken back from review: a new round of work
            task.pop("completed_at", None)
            task.pop("completed_by", None)
            task.pop("agent", None)  # the old name of started_by
            if agent:
                task["started_by"] = agent
        elif command == "done":
            task["completed_at"] = stamp
            if kwargs["resolution"] != "done":
                task["resolution"] = kwargs["resolution"]  # none means the default, done
            if agent:
                task["completed_by"] = agent
            if kwargs.get("commit"):
                task["commit"] = kwargs["commit"]
            # A batch task an agent finishes waits for the end-of-sprint review (SR-4.4): its note is what to check
            batch = old_kind == "active" and is_agent(agent) and effective_review(vocab, task) == "batch"
            if batch:
                task["sprint_review"] = "pending"
            if note:
                add_note(task, "review" if batch else "outcome", note, agent, stamp)


def change_status(
    task,
    data,
    command,
    reason=None,
    agent=None,
    status=None,
    note=None,
    commit=None,
    waiting_on=None,
    questions=(),
    answers=(),
    resolution=None,
    duplicate_of=None,
    *,
    archived,
):
    """Apply one status command to `task` (in `data`); raises AtmError if the transition isn't allowed. `archived` is
    the project's lookup of archived tasks (`start` needs every dependency done; one only in the archive is). `status`
    picks the new status when its kind has several; `agent` is who runs it. `start` records them as started_by,
    `done` as completed_by; `note` and `reason` become notes, `commit` the task's commit. `block` records who it
    waits on and the open questions; `unblock` turns them, with the answers, into an answer note. `done` and `skip`
    record a `resolution` (SR-2.8), `skip` also `duplicate_of` (the caller checks that task exists)."""
    # Each question was checked when the task was blocked, each answer when it is given: together they may be longer
    # than one note, and must still be recordable, or the task could never be unblocked
    asked = list(task.get("questions") or [])
    answered = answer_note(asked, list(answers)) if command == "unblock" and (asked or answers) else None
    machine = TaskMachine(task, data, agent, archived, now_iso())
    return machine.transition(
        command,
        reason=reason,
        status=status,
        note=note,
        commit=commit,
        waiting_on=waiting_on,
        questions=questions,
        answers=answers,
        resolution=resolution,
        duplicate_of=duplicate_of,
        answered=answered,
    )


# ==== lib/loop.py ====
# atm-lib: the loop's rules (docs/parallel-agents-design.md; docs/sprints-and-releases-design.md §3): leases, who
# holds a claim, what was decided in another worktree, and which task the loop takes next (`next_task`, the same order
# the dashboard's Next up shows). They work on the task data and the claims store as the store hands them out
# (`work.document`, `work.claims_store`), with the project's lookup of archived tasks (`archived`: a dependency only
# in the archive is done); keeping the claims is the store's (db/jsongit/claims.py).

DEFAULT_LEASE_MINUTES = 30


def archived_in(work):
    """The lookup of archived tasks inside a store change (`archived(task_id)` -> task or None): the change's own
    archive, read only when something asks for it."""
    index = {}

    def lookup(task_id):
        if not index:
            index.update({t.get("id"): t for t in work.archive()["tasks"] if isinstance(t, dict)} or {None: None})
        return index.get(task_id)

    return lookup


def apply_resolutions(worktree, data, store, archived):
    """Apply what was decided in another worktree about a task waiting in this one (store["resolved"]): the answer,
    the passed or rejected review, the skip, as if given here, so this worktree's copy stops waiting too. `worktree` is
    this working copy's folder, `archived` the project's lookup of archived tasks. A task changed here in the meantime
    is left as it is. Returns the IDs applied."""
    applied = []
    for task_id, record in list(store["resolved"].items()):
        if not isinstance(record, dict) or record.get("worktree") != worktree:
            continue
        store["resolved"].pop(task_id)
        task = next((t for t in data["tasks"] if t.get("id") == task_id), None)
        if task is None:
            continue
        try:
            change_status(
                task,
                data,
                record.get("command"),
                agent=record.get("by"),
                reason=record.get("reason"),
                answers=record.get("answers") or [],
                archived=archived,
            )
            applied.append(task_id)
        except (AtmError, KeyError):
            pass
    return applied


def lease_minutes(data):
    try:
        minutes = float(data.get("meta", {}).get("lease_minutes", DEFAULT_LEASE_MINUTES))
    except (TypeError, ValueError, AttributeError):
        return DEFAULT_LEASE_MINUTES
    return minutes if minutes > 0 else DEFAULT_LEASE_MINUTES


def lease_until(data):
    return (now() + timedelta(minutes=lease_minutes(data))).strftime("%Y-%m-%dT%H:%M:%SZ")


def drop_expired_claims(data, claims):
    """Drop claims whose lease ran out; their tasks in this worktree go back to todo. Returns the expired claims."""
    current = now()
    vocab = vocabulary(data)
    expired = []
    for task_id, claim in list(claims.items()):
        try:
            alive = parse_iso(claim["lease_expires_at"]) > current
        except (KeyError, TypeError, ValueError):
            alive = False
        if alive:
            continue
        expired.append(claims.pop(task_id))
        task = next((t for t in data["tasks"] if t.get("id") == task_id), None)
        if task and kind_of(vocab, task.get("status")) == "active":
            stamp = now_iso()
            back_to_todo(task, vocab, None, stamp, keep_commit=True)
            add_note(task, "failure", f"lease expired ({claim.get('agent')})", None, stamp)
    return expired


def check_holder(claims, task_id, agent, command):
    claim = claims.get(task_id)
    if claim and claim.get("agent") != agent:
        raise AtmError(
            f"can't {command} {task_id}: it is claimed by {claim.get('agent')} until {claim.get('lease_expires_at')}",
            4,
        )


def own_lease_expired(expired, data, task_id, agent, then):
    """When `agent`'s own claim on `task_id` is among the claims that just expired and the task went back to todo: an
    error that says so and how to go on (`then`, the command to run after starting it again). Exit code 4, like a claim
    conflict. Another agent's expired claim, or none: None."""
    claim = next((c for c in expired if c.get("task") == task_id and c.get("agent") == agent), None)
    task = next((t for t in data["tasks"] if t.get("id") == task_id), None)
    if not claim or not task or kind_of(vocabulary(data), task.get("status")) != "todo":
        return None
    until = str(claim.get("lease_expires_at", ""))
    at = f"{until[11:16]} UTC" if len(until) >= 16 else until
    minutes = lease_minutes(data)
    idle = f"{minutes:g} min"
    kept = " Your commit is kept." if task.get("commit") else ""
    next_steps = f"Run `atm task state {task_id} in_progress`" +(f", then `{then}` again." if then else " to claim it again.")
    return AtmError(
        f"{task_id}: your lease expired at {at} (no heartbeat for {idle}), so it went back to todo.{kept} {next_steps}",
        4,
    )


def ready_in_order(data, store, candidates=None, rank=None, *, archived):
    """The to-do tasks the loop could take, in the order it takes them: every dependency done (`archived`: the
    project's lookup of archived tasks, which are), not claimed, not completed or waiting in another worktree; by
    `rank` (the selection's own order, SR-3.2), then the most urgent priority, then lowest ID."""
    vocab = vocabulary(data)
    if candidates is None:
        candidates = [t for t in data["tasks"] if kind_of(vocab, t.get("status")) == "todo"]
    taken = set(store["claims"]) | set(store["completed"]) | set(store["waiting"])
    ready = [t for t in candidates if not unfinished_dependencies(data, t, archived) and t.get("id") not in taken]
    rank = rank or (lambda t: 0)
    order = lambda t: (rank(t), priority_rank(vocab, t.get("priority")), natural_key(t.get("id")))
    return sorted(ready, key=order)


class Selection:
    """What the loop works on (docs/sprints-and-releases-design.md §3.2): the active sprint (the default in a project
    with sprints), named sprints, a list of tasks, all tasks, or the backlog; `task_type` narrows it."""

    def __init__(self, sprints=(), tasks=(), all_tasks=False, backlog=False, task_type=None):
        self.sprints, self.tasks, self.all, self.backlog = list(sprints or []), list(tasks or []), all_tasks, backlog
        self.task_type = task_type


def select(data, sel):
    """The tasks `sel` covers, in any status, with how to rank them and a word for them. Returns (tasks, rank, scope,
    reason): reason is set when the selection can't be made (no active sprint, a closed sprint)."""
    vocab = vocabulary(data)
    sprints = vocab["sprints"]
    in_sprint = lambda names: (lambda t: (find_sprint(vocab, t.get("sprint")) or {}).get("id") in names)
    rank, scope, reason = None, "the tasks", None
    if sel.tasks:
        order = {task_id: i for i, task_id in enumerate(sel.tasks)}
        missing = [i for i in sel.tasks if i not in {t.get("id") for t in data["tasks"]}]
        if missing:
            raise AtmError(f"--tasks: no task {', '.join(missing)}", 2)
        keep, rank, scope = (lambda t: t.get("id") in order), (lambda t: order[t.get("id")]), "the listed tasks"
    elif sel.all:
        # The active sprint, then the planned ones in order, then the backlog (closed sprints' leftovers before it)
        place = {
            s["id"]: 0 if s["state"] == "active" else 1 + i if s["state"] == "planned" else len(sprints) + 1
            for i, s in enumerate(sprints)
        }
        keep = lambda t: True
        rank = lambda t: place.get((find_sprint(vocab, t.get("sprint")) or {}).get("id"), len(sprints) + 2)
        scope = "all tasks"
    elif sel.backlog:
        keep, scope = (lambda t: not find_sprint(vocab, t.get("sprint"))), "the backlog"
    elif sel.sprints:
        named = []
        for name in sel.sprints:
            sprint = find_sprint(vocab, name)
            if sprint is None:
                raise AtmError(f"no sprint {name!r}; the sprints: {', '.join(s['id'] for s in sprints) or 'none'}", 2)
            if sprint["state"] == "closed":
                raise AtmError(f"the sprint {sprint['id']!r} is closed", 2)
            named.append(sprint["id"])
        keep, scope = in_sprint(named), "the sprint " + " and ".join(f"“{n}”" for n in named)
    elif sprints:
        active = [s["id"] for s in sprints if s["state"] == "active"]
        if not active:
            return [], None, "the active sprint", "no active sprint: plan one with /atm:plan-sprint, or run with --all"
        keep, scope = in_sprint(active), f"the sprint “{active[0]}”"
    else:
        keep = lambda t: True  # a project without sprints: every task, as before sprints
    tasks = [
        t
        for t in data["tasks"]
        if keep(t)
        and (sel.task_type is None or t.get("type") == sel.task_type)
    ]
    return tasks, rank, scope, reason


def offered_to(data, task, agent):
    """Whether a task back from review (rework or approved) may go to `agent`: always to the agent that worked on it;
    to others once one lease length has passed since the review ended (SR-3.9, Q11)."""
    if not agent or task.get("started_by") == agent:
        return True
    try:
        reviewed = parse_iso(task.get("reviewed_at"))
    except (TypeError, ValueError):
        return True
    return now() >= reviewed + timedelta(minutes=lease_minutes(data))


def loop_order(data, store, sel, agent=None, with_user=False, *, archived):
    """Every task the loop could take now, in the order it takes them: `atm next` hands out the first, and atm serve's
    summary lists them as `next`, which the dashboard's Next up shows (T-321), so the two can't disagree. Returns
    (ready, selected, candidates, together, scope, problem); `problem` says why the selection itself is refused."""
    vocab = vocabulary(data)
    selected, rank, scope, problem = select(data, sel)
    if problem:
        return [], selected, [], [], scope, problem
    # Tasks back from review come first (SR-3.9): the agent that worked on them gets them back for a lease length
    rework = [
        t
        for t in selected
        if kind_of(vocab, t.get("status")) == "review"
        and t.get("review_state") in REWORK_STATES
        and offered_to(data, t, agent)
    ]
    first = {t.get("id") for t in rework}
    candidates = rework + [t for t in selected if kind_of(vocab, t.get("status")) == "todo"]
    # Work done together with the user waits for them, unless they're watching (SR-3.5)
    together = [t for t in candidates if t.get("with") == "user"]
    if not with_user:
        candidates = [t for t in candidates if t.get("with") != "user"]
    in_order = rank or (lambda t: 0)
    ready = ready_in_order(
        data, store, candidates, lambda t: (t.get("id") not in first, in_order(t)), archived=archived
    )
    return ready, selected, candidates, together, scope, None


def next_task(data, store, task_type=None, sel=None, agent=None, with_user=False, *, archived):
    """The loop's rule (same as the dashboard's "Next up"): todo (or back from review), every dependency done, not
    claimed and not completed in another worktree, in the selection (the default one when `sel` is None); in its
    order, then the most urgent priority, then lowest ID. Returns (task, reason) where reason explains an empty
    result: the selection is finished, the rest of it waits, or it is empty (SR-3.6)."""
    vocab = vocabulary(data)
    sel = sel or Selection(task_type=task_type)
    ready, selected, candidates, together, scope, problem = loop_order(
        data, store, sel, agent, with_user, archived=archived
    )
    if problem:
        return None, problem
    if ready:
        return ready[0], None
    if together and not with_user and not candidates:
        ids = ", ".join(t.get("id") for t in together)
        return None, f"only work done together with the user is left in {scope}: {ids} (--with-user takes it)"
    if not candidates:
        if not selected:
            if not vocab["sprints"]:
                return None, "nothing left to do"
            return None, f"{scope} {'has' if scope.startswith('the sprint') else 'have'} no tasks"
        kinds = [kind_of(vocab, t.get("status")) for t in selected]
        open_kinds = {k: kinds.count(k) for k in ("active", "review", "blocked") if k in kinds}
        if open_kinds:
            words = {"active": "in progress", "review": "in review", "blocked": "blocked"}
            what = ", ".join(f"{n} {words[k]}" for k, n in open_kinds.items())
            return None, f"nothing ready in {scope}: the rest is waiting ({what})"
        if vocab["sprints"]:
            return None, f"{scope} is finished: {kinds.count('done')} done" + (
                f", {kinds.count('skipped')} skipped" if kinds.count("skipped") else ""
            )
        return None, "nothing left to do"
    claimed = [t["id"] for t in candidates if t.get("id") in store["claims"]]
    done_elsewhere = [t["id"] for t in candidates if t.get("id") in store["completed"]]
    waiting_elsewhere = [t["id"] for t in candidates if t.get("id") in store["waiting"]]
    reason = f"{len(candidates)} todo task(s) waiting on dependencies"
    if claimed:
        reason += f" or claimed by other agents ({', '.join(claimed)})"
    if done_elsewhere:
        reason += f"; done in another worktree, not merged yet: {', '.join(done_elsewhere)}"
    if waiting_elsewhere:
        reason += f"; blocked or in review in another worktree: {', '.join(waiting_elsewhere)}"
    return None, reason


# ==== lib/tasks.py ====
# atm-lib: reading tasks (docs/layered-architecture.md §5.1).

def view_task(task, archived, tasks, archived_lookup, vocab):
    """A task with what is derived from the rest of the project: its effective priority, its dependencies' statuses,
    the tasks related to it either way (`related` is stored on one side, SR-2.8) and those that duplicate it.

    `tasks` are the working tasks, `archived_lookup(id)` an archived task or None, `vocab` the vocabularies."""
    by_id = {t.get("id"): t for t in tasks}
    dependencies = []
    for d in task.get("dependencies") or []:
        if d in by_id:
            dependencies.append({"id": d, "status": by_id[d].get("status"), "archived": False})
        elif archived_lookup(d):
            dependencies.append({"id": d, "status": archived_lookup(d).get("status"), "archived": True})
        else:
            dependencies.append({"id": d, "status": None, "archived": False})
    own = [r for r in task.get("related") or [] if isinstance(r, str)]
    others = [
        t.get("id")
        for t in tasks
        if t is not task and task.get("id") in (t.get("related") or []) and t.get("id") not in own
    ]
    return {
        "task": task,
        "archived": archived,
        "priority": priority_of(vocab, task),
        "started_by": started_by(task),
        "dependencies": dependencies,
        "related": own + others,
        "duplicated_by": [t.get("id") for t in tasks if t.get("duplicate_of") == task.get("id")],
    }


def show_tasks(project, ids):
    """`atm show`: the tasks in full, archived ones too, in the order asked for."""
    store = project.store
    tasks = store.list_tasks()
    vocab = store.get_vocabularies()
    views = []
    for task_id in ids:
        task, archived = store.get_task(task_id), False
        if task is None:
            task, archived = store.get_archived_task(task_id), True
        if task is None:
            raise NotFound(f"no task {task_id}")
        views.append(view_task(task, archived, tasks, store.get_archived_task, vocab))
    return views


# ==== lib/query.py ====
# atm-lib: the read commands (docs/layered-architecture.md §5.1): list, deps, status, the project's summary, the
# backlog. Agents read tasks through these, never tasks.json itself (DM-1.1). They never write: the store reads the
# task files and the claims without the lock (both are replaced atomically).
#
# Much of it still takes the whole task document (`store.document()`) for the rules it calls on it
# (`unfinished_dependencies`, `loop_order`, ...).

class Filters:
    """What a read keeps (`atm list`'s and `atm status`'s options): any of `sprint` (a list of names), `backlog`,
    `release`, `component`, `status` (a list), `type`, `priority`, `label` (a list: all of them), `agent`, `ready`,
    `blocked`, `in_pr`, `with_user`, `archived`; what isn't given keeps everything. The CLI passes its parsed
    arguments, which have these names."""

    def __init__(self, **given):
        self.__dict__.update(given)

    def __getattr__(self, name):
        return None


def summary(vocab, task):
    """The fields `list` shows, as a dict for --json."""
    fields = {
        "id": task.get("id"),
        "status": task.get("status"),
        "kind": kind_of(vocab, task.get("status")),
        "sprint": task.get("sprint"),
        "priority": priority_of(vocab, task),
        "labels": task.get("labels", []),
        "name": task.get("name", ""),
    }
    if isinstance(task.get("pr"), dict):
        fields["pr"] = task["pr"]
        fields["merged_at"] = task.get("merged_at")
    return fields


def scope_filter(vocab, args):
    """A test for the tasks `--sprint` (repeatable), `--backlog`, `--release` and `--component` keep (SR-3.2); a
    task's release is its effective one (its own, else its sprint's)."""
    names = []
    for name in getattr(args, "sprint", None) or []:
        sprint = find_sprint(vocab, name)
        if sprint is None:
            raise NotFound(f"no sprint {name!r}; the sprints: {', '.join(s['id'] for s in vocab['sprints']) or 'none'}", 2)
        names.append(sprint["id"])
    if getattr(args, "release", None):
        known_release(vocab, args.release)

    def keep(task):
        sprint = (find_sprint(vocab, task.get("sprint")) or {}).get("id")
        if names and sprint not in names:
            return False
        if getattr(args, "backlog", False) and sprint:
            return False
        if getattr(args, "release", None) and effective_release(vocab, task) != args.release:
            return False
        return not getattr(args, "component", None) or args.component in (task.get("components") or [])

    return keep


def in_open_pr(task):
    """In PR: a recorded PR, not merged yet (DM-4.3)."""
    return isinstance(task.get("pr"), dict) and not task.get("merged_at")


def ready_tasks(data, archived):
    """To-do tasks whose dependencies are all done, in the loop's order. `archived`: the project's lookup of archived
    tasks (a dependency only in the archive is done)."""
    vocab = vocabulary(data)
    ready = [
        t
        for t in data["tasks"]
        if kind_of(vocab, t.get("status")) == "todo" and not unfinished_dependencies(data, t, archived)
    ]
    return sorted(ready, key=lambda t: (priority_rank(vocab, t.get("priority")), natural_key(t.get("id"))))


def list_matching(project, filters):
    """`atm list`: the tasks the filters keep, in the project's order (the archive's with `archived`)."""
    store = project.store
    data = store.document()
    vocab = vocabulary(data)
    args = filters
    ready = {t.get("id") for t in ready_tasks(data, store.get_archived_task)}
    in_scope = scope_filter(vocab, args)
    tasks = []
    for t in store.list_archived() if args.archived else data["tasks"]:
        kind = kind_of(vocab, t.get("status"))
        if not in_scope(t):
            continue
        if args.status and t.get("status") not in args.status:
            continue
        if args.type and t.get("type") != args.type:
            continue
        if args.priority and priority_of(vocab, t) != args.priority:
            continue
        if args.label and not set(args.label) <= set(t.get("labels") or []):
            continue
        if args.agent and started_by(t) != args.agent:
            continue
        if args.ready and t.get("id") not in ready:
            continue
        if args.blocked and not (kind == "blocked" or (kind == "todo" and t.get("id") not in ready)):
            continue
        if args.in_pr and not in_open_pr(t):
            continue
        if args.with_user and t.get("with") != "user":
            continue
        tasks.append(t)
    return {"vocab": vocab, "tasks": tasks}


def task_stage(vocab, task):
    """A done task's stage (DM-4.3): `done`, `in_pr` (a recorded PR, not merged) or `merged`; None for other kinds."""
    if kind_of(vocab, task.get("status")) != "done":
        return None
    if task.get("merged_at"):
        return "merged"
    return "in_pr" if isinstance(task.get("pr"), dict) else "done"


def task_views(project, filters):
    """The tasks the filters keep, each with what atm derives for it (the REST API's tasks, docs/layered-architecture.md
    §6.2): its kind, whether it is ready, the dependencies it waits for, who a blocked task waits on, its effective
    priority, review mode and release, its stage, and whether it is archived. The dashboard takes these instead of
    working them out itself."""
    found = list_matching(project, filters)
    data, vocab, archived = project.store.document(), found["vocab"], project.store.get_archived_task
    ready = {t.get("id") for t in ready_tasks(data, archived)}
    # A task a tolerant read found wrong (T-377) says so; what its broken fields would decide isn't decided
    troubled = {} if filters.archived else project.store.problems()["tasks"]
    views = []
    for t in found["tasks"]:
        kind = kind_of(vocab, t.get("status"))
        problems = troubled.get(t.get("id"))
        values = {
            "kind": kind,
            "ready": t.get("id") in ready and not problems,
            "waiting_for": [d for d, _ in unfinished_dependencies(data, t, archived)] if kind == "todo" else [],
            "problems": problems,
            "waiting_on": t.get("waiting_on") if kind == "blocked" else None,
            "priority": priority_of(vocab, t),
            "effective_review": effective_review(vocab, t),
            "effective_release": effective_release(vocab, t),
            "stage": task_stage(vocab, t),
            "archived": bool(filters.archived),
        }
        # What doesn't apply is left out, not null (api.schema.json's optional fields)
        views.append({"task": t, "derived": {k: v for k, v in values.items() if v is not None}})
    return views


def read_problems(project):
    """What a tolerant read found wrong with the project's task data as a whole (T-377): entries left out, the
    vocabularies that fell back to their defaults. Empty for a strict read, which refuses such data instead."""
    return list(project.store.problems()["file"])


def search_text(task):
    """What a task is found by: its ID, name, description, type, labels and notes (T-258)."""
    strings = [task.get(f) for f in ("id", "name", "description", "type")]
    strings += task.get("labels") if isinstance(task.get("labels"), list) else []
    strings += [n.get("text") for n in task.get("notes") or [] if isinstance(n, dict)]
    return " ".join(s for s in strings if isinstance(s, str)).lower()


def search_tasks(project, query, also=""):
    """The IDs of the working tasks (not the archive) in which every word of `query` appears, case-insensitively, in
    the task's text or in `also` (the project's name, so it finds all of its tasks), in the project's order."""
    terms = query.lower().split()
    if not terms:
        return []
    extra = also.lower()
    return [
        t["id"]
        for t in project.store.list_tasks()
        if isinstance(t.get("id"), str) and all(term in f"{extra} {search_text(t)}" for term in terms)
    ]


def list_claims(project):
    """The project's claims, from every working copy, by task."""
    claims = project.store.read_claims()["claims"]
    listed = [c for c in claims.values() if isinstance(c, dict) and isinstance(c.get("task"), str)]
    return sorted(listed, key=lambda c: c["task"])


def task_dependencies(project, task_id):
    """`atm deps`: what a task waits for, and what waits for it, archived tasks included (T-358)."""
    store = project.store
    task = store.get_task(task_id) or store.get_archived_task(task_id)
    if task is None:
        raise NotFound(f"no task {task_id}")
    by_id = {t.get("id"): t for t in store.list_tasks()}

    def dependency(d):
        found = by_id.get(d) or store.get_archived_task(d)
        if not found:
            return {"id": d, "status": "missing", "name": ""}
        return {"id": d, "status": found.get("status") + (", archived" if d not in by_id else ""), "name": found.get("name", "")}

    waits_for = [dependency(d) for d in task.get("dependencies") or []]
    waited_on_by = [
        {"id": t.get("id"), "status": t.get("status"), "name": t.get("name", "")}
        for t in store.list_tasks()
        if task_id in (t.get("dependencies") or [])
    ]
    # Archived tasks that depended on it are history too, marked as the waits-for side marks them (T-358); a task in
    # both files is the tasks.json one
    waited_on_by += [
        {"id": t.get("id"), "status": f"{t.get('status')}, archived", "name": t.get("name", "")}
        for t in store.list_archived()
        if t.get("id") not in by_id and isinstance(t.get("dependencies"), list) and task_id in t["dependencies"]
    ]
    return {"id": task_id, "waits_for": waits_for, "waited_on_by": waited_on_by}


ACTIVITY_FIELDS = ("updated_at", "started_at", "completed_at", "review_requested_at", "blocked_at", "skipped_at")
SUMMARY_DAYS = 14


def parse_time(text):
    try:
        return parse_iso(text) if isinstance(text, str) else None
    except ValueError:
        return None


RECENT_SPRINTS = 4
NEXT_TASKS = 2
NAME_CAP = 80  # task names in the summary, so /api/projects stays small with many projects


def capped(name):
    name = name if isinstance(name, str) else ""
    return name if len(name) <= NAME_CAP else name[: NAME_CAP - 1] + "…"


def sprint_numbers(sprint, own_kinds):
    """A sprint as the overview shows it: its name, state, due date and release, and done of its tasks but the skipped."""
    done, total = own_kinds.count("done"), len(own_kinds) - own_kinds.count("skipped")
    return {
        "id": sprint["id"],
        "state": sprint["state"],
        "due": sprint.get("due"),
        "release": sprint.get("release"),
        "done": done,
        "total": total,
        "percent": round(100 * done / total) if total else 100,
    }


def recent_sprints(vocab, tasks, kinds):
    """Up to four sprints with tasks for the overview's rings (SR-7.7), in the project's order: the active one and the
    planned ones first, then the most recently closed (by their end) to fill up. None for a project without sprints."""
    if not vocab["sprints"]:
        return None
    own = {s["id"]: [] for s in vocab["sprints"]}
    for t, k in zip(tasks, kinds):
        sprint = find_sprint(vocab, t.get("sprint"))
        if sprint:
            own[sprint["id"]].append(k)
    with_tasks = [s for s in vocab["sprints"] if own[s["id"]]]
    chosen = [s["id"] for s in with_tasks if s["state"] != "closed"][:RECENT_SPRINTS]
    closed = sorted((s for s in with_tasks if s["state"] == "closed"), key=lambda s: s.get("end") or "", reverse=True)
    chosen += [s["id"] for s in closed[: RECENT_SPRINTS - len(chosen)]]
    return [sprint_numbers(s, own[s["id"]]) for s in with_tasks if s["id"] in chosen]


def active_sprint_numbers(vocab, tasks, kinds):
    """The active sprint for the overview's cards (SR-7.7), with its progress; None when none is active."""
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)
    if active is None:
        return None
    own = [k for t, k in zip(tasks, kinds) if (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]]
    return {**sprint_numbers(active, own), "goal": active.get("goal")}


DESCRIPTION_CAP = 200


def description(meta):
    """meta.description (an optional line about the project, set with atm init --meta-file), capped at 200 characters
    for the overview, where it is the project name's tooltip (T-209); None without one."""
    text = meta.get("description") if isinstance(meta, dict) else None
    if not isinstance(text, str) or not text.strip():
        return None
    text = text.strip()
    return text if len(text) <= DESCRIPTION_CAP else text[: DESCRIPTION_CAP - 1] + "…"


def git_summary(project, tasks):
    """The project's git line for its card (T-206): the main worktree's branch (read from .git/HEAD, no git process),
    and the latest PR recorded on its tasks (`atm pr`; the highest number), `merged` once its tasks have merged_at.
    No forge call and no CI state."""
    branch = project.vcs.branch() if project.vcs else None
    prs = {}
    for t in tasks:
        pr = t.get("pr")
        if isinstance(pr, dict) and isinstance(pr.get("number"), int):
            entry = prs.setdefault(pr["number"], {"number": pr["number"], "url": None, "state": "open"})
            # The dashboard links to it: only a web address (docs/security-review.md F9)
            if isinstance(pr.get("url"), str) and re.match(r"https?://\S+\Z", pr["url"], re.IGNORECASE):
                entry["url"] = pr["url"]
            if t.get("merged_at"):
                entry["state"] = "merged"
    return {"branch": branch, "pr": prs[max(prs)] if prs else None}


BOARD_CAP = 30  # cards per column of the All projects board (docs/all-projects-kanban-proposal.md, 3.1)
BOARD_READY_CAP = 10  # to-do tasks per project, so one project's new batch doesn't fill the To Do column
BOARD_DONE_HOURS = 24


def board_card(vocab, task, **extra):
    """What a project's Kanban card shows, nothing more (T-257)."""
    labels = task.get("labels")
    card = {
        "id": task.get("id"),
        "name": capped(task.get("name")),
        "type": task.get("type"),
        "sprint": task.get("sprint") if isinstance(task.get("sprint"), str) else None,
        "priority": priority_of(vocab, task),
        "labels": [l for l in labels if isinstance(l, str)] if isinstance(labels, list) else [],
    }
    card.update(extra)
    return card


def board_column(cards, cap=BOARD_CAP):
    """A column: its first `cap` cards, and how many there are in all (for "+ N more")."""
    return {"count": len(cards), "cards": cards[:cap]}


def todo_order(data, tasks, kinds, loop_ready, vocab, archived):
    """Every to-do task, for the board's To Do column (T-349): first the ready ones, those the loop takes in its order
    before the rest (other sprints', the backlog's), then the ready ones done with the user, then those waiting on
    dependencies (each by priority, then ID). Each with why it isn't plain ready: done with the user, or the
    dependencies it waits for."""
    rank = {t.get("id"): i for i, t in enumerate(loop_ready)}

    def entry(t):
        waits = [dep for dep, _ in unfinished_dependencies(data, t, archived)]
        key = (
            bool(waits),
            t.get("with") == "user",
            t.get("id") not in rank,
            rank.get(t.get("id"), 0),
            priority_rank(vocab, t.get("priority")),
            natural_key(t.get("id")),
        )
        return key, t, waits

    entries = sorted((entry(t) for t, k in zip(tasks, kinds) if k == "todo"), key=lambda e: e[0])
    return [(t, waits) for _, t, waits in entries]


def board(data, store, tasks, kinds, todo, vocab, current, live, recent_done=True):
    """The project's cards for the All projects board (T-257), one rule per column: every to-do task (`todo`, as
    todo_order gives it, T-349); everything in progress (longest running first), in review (longest waiting first)
    and blocked (on the user first, then longest waiting); what was finished, newest first: in the last 24 hours, or
    all of it without `recent_done` (a sprint's board shows all its tasks, T-349)."""
    of_kind = lambda kind: [t for t, k in zip(tasks, kinds) if k == kind]
    claimed_by = {c.get("task"): c.get("agent") for c in live}
    since = lambda t, field: t.get(field) or t.get("updated_at")
    active = sorted(of_kind("active"), key=lambda t: str(t.get("started_at") or ""))
    review = sorted(of_kind("review"), key=lambda t: str(since(t, "review_requested_at") or ""))
    blocked = sorted(
        of_kind("blocked"),
        key=lambda t: (t.get("waiting_on") != "user", str(since(t, "blocked_at") or "")),
    )
    cutoff = current.timestamp() - BOARD_DONE_HOURS * 3600
    finished_at = lambda t: parse_time(t.get("completed_at"))
    done = [
        t for t in of_kind("done") if not recent_done or (finished_at(t) and finished_at(t).timestamp() > cutoff)
    ]
    done.sort(key=lambda t: str(t.get("completed_at")), reverse=True)
    return {
        "todo": board_column(
            [
                board_card(
                    vocab,
                    t,
                    **({"with_user": True} if t.get("with") == "user" else {}),
                    **({"waiting_for": waits} if waits else {}),
                )
                for t, waits in todo
            ],
            BOARD_READY_CAP,
        ),
        "active": board_column(
            [
                board_card(vocab, t, by=claimed_by.get(t.get("id")) or started_by(t), since=since(t, "started_at"))
                for t in active
            ]
        ),
        "review": board_column([board_card(vocab, t, since=since(t, "review_requested_at")) for t in review]),
        "blocked": board_column(
            [
                board_card(
                    vocab,
                    t,
                    since=since(t, "blocked_at"),
                    waiting_on=t.get("waiting_on"),
                    questions=len(t.get("questions") or []),
                )
                for t in blocked
            ]
        ),
        "done": board_column([board_card(vocab, t, completed_at=t.get("completed_at")) for t in done]),
    }


def work_span(vocab, task, current):
    """A task's work as (start, end) timestamps, as the dashboard counts it (work.ts): from started_at until it asked
    for a review or was done, or until now while active; None for a task that holds none."""
    start = parse_time(task.get("started_at"))
    if not start:
        return None
    kind = kind_of(vocab, task.get("status"))
    if kind == "active":
        end = max(current, start)
    elif kind in ("done", "review"):
        end = (
            parse_time(task.get("review_requested_at"))
            or parse_time(task.get("completed_at") if kind == "done" else task.get("updated_at"))
            or start
        )
    else:
        return None
    return (start.timestamp(), max(end, start).timestamp())


def agent_minutes(vocab, tasks, current):
    """Agent time (docs/project-stats-proposal.md, T-255): the work of tasks an agent started, or nobody recorded
    (before v4), each agent's spans merged so an agent on two tasks counts once; parallel agents add up. As the
    dashboard's Project tab counts it (projectstats.ts)."""
    spans = {}
    for i, t in enumerate(tasks):
        starter = started_by(t)
        if starter and not str(starter).startswith("claude:"):
            continue
        span = work_span(vocab, t, current)
        if span:
            spans.setdefault(starter or i, []).append(span)  # unattributed spans are each their own
    total = 0.0
    for agent_spans in spans.values():
        end = None
        for s, e in sorted(agent_spans):
            if end is None or s > end:
                total += e - s
                end = e
            elif e > end:
                total += e - end
                end = e
    return round(total / 60)


def project_stats(store, data, vocab, tasks, current):
    """The overview card's line (T-255): when the project started (its first task's created_at, else meta.created),
    how many tasks it has, and the agent time, archived tasks included; an unreadable archive is left out. (A task
    folder with a symlinked file never gets here: find_paths refuses it, docs/security-review.md F8.)"""
    try:
        archived = store.list_archived()
    except AtmFailure:
        archived = []
    ids = {t.get("id") for t in tasks}
    every = tasks + [t for t in archived if t.get("id") not in ids]
    created = [parse_time(t.get("created_at")) for t in every]
    created = [c for c in created if c] or [parse_time((data.get("meta") or {}).get("created"))]
    started = min((c for c in created if c), default=None)
    return {
        "started": started.strftime("%Y-%m-%dT%H:%M:%SZ") if started else None,
        "tasks": len(every),
        "agent_minutes": agent_minutes(vocab, every, current),
    }


def project_summary(project):
    """A project's numbers for the dashboard's overview and project selector (docs/multi-project-api.md, 3.2), by
    status kind as atm counts them everywhere else: done / (total - skipped), the active and recent sprints, live
    claims, what needs the user, the latest activity and the completions of the last 14 days."""
    data, archived = project.store.document(), project.store.get_archived_task
    vocab = vocabulary(data)
    tasks = data["tasks"]
    kinds = [kind_of(vocab, t.get("status")) for t in tasks]
    done, skipped = kinds.count("done"), kinds.count("skipped")
    counted = len(tasks) - skipped
    percent = round(100 * done / counted) if counted else (100 if skipped else 0)

    current = now()
    store = project.store.read_claims()
    live = sorted(
        (
            c
            for c in store["claims"].values()
            if isinstance(c, dict) and (parse_time(c.get("lease_expires_at")) or current) > current
        ),
        key=lambda c: str(c.get("claimed_at") or ""),
    )
    agents_working = len(live)
    by_id = {t.get("id"): t for t in tasks}
    # What the cards show by name (T-205): who works on what now, what the loop takes next, the last finished task
    working = [
        {
            "task": c.get("task"),
            "name": capped((by_id.get(c.get("task")) or {}).get("name")),
            "agent": c.get("agent"),
            "branch": c.get("branch"),
        }
        for c in live
    ]
    # What the loop takes next with its default selection (the active sprint's tasks in a project with sprints, SR-5.1),
    # by the very rule `atm next` follows: the dashboard's Next up shows the first (T-321)
    in_loop_order = loop_order(data, store, Selection(), archived=archived)[0]
    next_up = [{"id": t.get("id"), "name": capped(t.get("name"))} for t in in_loop_order[:NEXT_TASKS]]
    # The board's To Do column: every to-do task, the loop's first (T-349)
    todo = todo_order(data, tasks, kinds, in_loop_order, vocab, archived)
    finished = [(t.get("completed_at"), t) for t, k in zip(tasks, kinds) if k == "done" and parse_time(t.get("completed_at"))]
    last_task = max(finished, key=lambda f: f[0])[1] if finished else None
    last_done = (
        {"id": last_task.get("id"), "name": capped(last_task.get("name")), "completed_at": last_task.get("completed_at")}
        if last_task
        else None
    )

    times = [parse_time(t.get(f)) for t in tasks for f in ACTIVITY_FIELDS]
    times += [parse_time(n.get("at")) for t in tasks for n in t.get("notes") or [] if isinstance(n, dict)]
    times = [t for t in times if t]
    last = max(times) if times else None

    completions = [0] * SUMMARY_DAYS
    today = current.date()
    for t, k in zip(tasks, kinds):
        finished = parse_time(t.get("completed_at")) if k == "done" else None
        if finished:
            age = (today - finished.date()).days
            if 0 <= age < SUMMARY_DAYS:
                completions[SUMMARY_DAYS - 1 - age] += 1

    # The tasks that need the user, as the dashboard's needsYouReason decides it: in review and waiting for the
    # reviewer, blocked on the user, or to do together with the user (SR-6.6) now: in the active sprint (any, without
    # one) and ready, its dependencies done (T-347)
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)

    def together_now(t):
        in_scope = active is None or (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]
        return in_scope and not unfinished_dependencies(data, t, archived)

    def needs_you(t, k):
        if k == "review":
            return None if t.get("review_state") in REWORK_STATES else "review"
        if k == "blocked" and t.get("waiting_on") == "user":
            return "blocked"
        return "together" if k == "todo" and t.get("with") == "user" and together_now(t) else None

    needs_you_tasks = [
        {"id": t.get("id"), "name": capped(t.get("name")), "reason": needs_you(t, k)}
        for t, k in zip(tasks, kinds)
        if needs_you(t, k)
    ]

    return {
        "total": len(tasks),
        "done": done,
        "skipped": skipped,
        "percent": percent,
        "active_sprint": active_sprint_numbers(vocab, tasks, kinds),
        "recent_sprints": recent_sprints(vocab, tasks, kinds),
        "agents_working": agents_working,
        "working": working,
        "next": next_up,
        "last_done": last_done,
        "needs_you": {
            "review": sum(1 for item in needs_you_tasks if item["reason"] == "review"),
            "blocked_on_user": sum(1 for item in needs_you_tasks if item["reason"] == "blocked"),
            "together": sum(1 for item in needs_you_tasks if item["reason"] == "together"),
        },
        "needs_you_tasks": needs_you_tasks,
        "git": git_summary(project, tasks),
        "description": description(data.get("meta")),
        "last_activity": last.strftime("%Y-%m-%dT%H:%M:%SZ") if last else None,
        "completions_14d": completions,
        "board": board(data, store, tasks, kinds, todo, vocab, current, live),
        "board_active_sprint": active_sprint_board(data, store, tasks, kinds, todo, vocab, current, live),
        "stats": project_stats(project.store, data, vocab, tasks, current),
    }


def active_sprint_board(data, store, tasks, kinds, todo, vocab, current, live):
    """The All projects board's default view (T-340): the same columns, of the active sprint's tasks only, no more, no
    less (T-349: every one done too, not the last 24 hours'), with their own counts; None without an active sprint (the
    board says so)."""
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)
    if active is None:
        return None
    in_sprint = lambda t: (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]
    pairs = [(t, k) for t, k in zip(tasks, kinds) if in_sprint(t)]
    return board(
        data,
        store,
        [t for t, _ in pairs],
        [k for _, k in pairs],
        [(t, waits) for t, waits in todo if in_sprint(t)],
        vocab,
        current,
        live,
        recent_done=False,
    )


def status_report(project, scope):
    """`atm status`: the overview, in the scope's tasks (`Filters` with sprint, backlog, release, component): the
    active sprint, counts, what is in progress, in review, blocked or waiting, the next task, the files' size and the
    hints. `report` is what --json prints; `has_sprints` says whether the project has any."""
    data, archived = project.store.document(), project.store.get_archived_task
    vocab = vocabulary(data)
    store = project.store.read_claims()
    sizes = project.store.sizes()
    args = scope
    in_scope = scope_filter(vocab, args)
    tasks = [t for t in data["tasks"] if in_scope(t)]
    active = next((s for s in vocab["sprints"] if s["state"] == "active"), None)
    sprint = None
    if active:
        in_active = [t for t in data["tasks"] if (find_sprint(vocab, t.get("sprint")) or {}).get("id") == active["id"]]
        sprint = {
            **{k: active[k] for k in ("id", "goal", "due", "release", "start") if active.get(k)},
            "done": sum(1 for t in in_active if kind_of(vocab, t.get("status")) == "done"),
            "tasks": sum(1 for t in in_active if kind_of(vocab, t.get("status")) != "skipped"),
            # Batch tasks waiting for the end-of-sprint review: they wait on the user (SR-4.4)
            "review_pending": [t.get("id") for t in in_active if t.get("sprint_review") == "pending"],
        }

    def counts(key):
        result = {}
        for t in tasks:
            for value in key(t):
                result[str(value)] = result.get(str(value), 0) + 1
        return result

    status_order = [s["id"] for s in vocab["statuses"]]
    by_status = counts(lambda t: [t.get("status")])
    by_status = {s: by_status[s] for s in status_order if s in by_status} | {
        s: n for s, n in by_status.items() if s not in status_order
    }
    in_progress = [
        {
            "id": t.get("id"),
            "name": t.get("name", ""),
            "by": (store["claims"].get(t.get("id")) or {}).get("agent") or started_by(t),
        }
        for t in tasks
        if kind_of(vocab, t.get("status")) == "active"
    ]
    waiting = [
        {"id": t.get("id"), "name": t.get("name", ""), "on": [d for d, _ in unfinished_dependencies(data, t, archived)]}
        for t in tasks
        if kind_of(vocab, t.get("status")) == "todo" and unfinished_dependencies(data, t, archived)
    ]
    blocked = [
        {"id": t.get("id"), "name": t.get("name", ""), "waiting_on": t.get("waiting_on"), "questions": t.get("questions", [])}
        for t in tasks
        if kind_of(vocab, t.get("status")) == "blocked"
    ]
    in_review = [
        {"id": t.get("id"), "name": t.get("name", ""), "since": t.get("review_requested_at")}
        for t in tasks
        if kind_of(vocab, t.get("status")) == "review"
    ]
    # Blocked or in review in another worktree (T-204): this worktree's copy doesn't say so until the branches merge
    task_names = {t.get("id"): t.get("name", "") for t in tasks}
    listed = {i["id"] for i in blocked + in_review}
    for record in store["waiting"].values():
        here = project.store.working_copy()
        if not isinstance(record, dict) or record.get("worktree") == here or record.get("task") in listed:
            continue
        item = {"id": record.get("task"), "name": task_names.get(record.get("task"), ""), "branch": record.get("branch")}
        if kind_of(vocab, record.get("status")) == "review":
            in_review.append({**item, "since": record.get("at")})
        else:
            blocked.append({**item, "waiting_on": record.get("waiting_on"), "questions": record.get("questions") or []})
    nxt, reason = next_task(data, store, archived=archived)
    largest = sorted(((len(json.dumps(t, ensure_ascii=False)), t.get("id")) for t in tasks), reverse=True)
    result = {
        "sprint": sprint,
        "tasks": len(tasks),
        "by_status": by_status,
        "by_sprint": counts(lambda t: [(find_sprint(vocab, t.get("sprint")) or {}).get("id") or "backlog"]),
        "by_type": counts(lambda t: [t.get("type")]),
        "by_label": counts(lambda t: t.get("labels") or []),
        "in_progress": in_progress,
        "in_review": in_review,
        "blocked": blocked,
        "waiting_on_dependencies": waiting,
        "next": {"id": nxt.get("id"), "name": nxt.get("name", "")} if nxt else None,
        "next_reason": reason,
        "file_bytes": sizes["tasks"],
        "largest_tasks": [{"id": task_id, "bytes": size} for size, task_id in largest[:3]],
        "archived": len(project.store.list_archived()),
        "archive_bytes": sizes["archive"],
    }
    # Open tasks left in closed sprints (T-330), whatever the scope: the loop's default selection never takes them
    stranded = open_in_closed_sprints(vocab, data["tasks"])
    if stranded:
        result["open_in_closed_sprints"] = stranded
    hint = archive_hint_for(data, sizes["tasks"])
    if hint:
        result["suggest"] = {"archive": hint}
    return {"report": result, "has_sprints": bool(vocab["sprints"])}


# ==== lib/status.py ====
# atm-lib: the status changes (start, review, done, reject, approve, request-changes, block, unblock, reopen, skip,
# fail, release) and notes (docs/layered-architecture.md §5.1).
#
# Each runs as one change of the store (`store.change`, §3.3): the task's status, its claim, and the records shared by
# a clone's worktrees (completed, waiting, resolved) change together or not at all. The transitions themselves are
# common's (`change_status`, common/model.py), the lease and resolution rules lib/loop.py's.

# The commands an agent runs on its own claimed task: if that claim just expired, they explain it instead of refusing
OWN_LEASE_COMMANDS = ("done", "release", "fail", "review", "block")

# The state `atm task state <id> <state>` names for the commands an agent runs on its own claim, for the hint that
# says what to run again
COMMAND_STATES = {"done": "done", "release": "todo", "fail": "todo", "review": "in_review", "block": "blocked"}

# Decisions that may be made in any worktree about a task waiting in another (T-204): the command, and the kinds the
# waiting task must have there
REMOTE_DECISIONS = {"unblock": ("blocked",), "skip": ("blocked",), "done": ("review",), "reject": ("review",)}


def todo_command(project, task_id, failing=False):
    """The status function `atm task state <id> todo` runs, by the kind the task is in: in progress, a failure
    (`failing`, it says why) or a release; in review, reject; blocked, unblock; done or skipped, reopen. A task that is
    to do here but claimed (in progress in another worktree, T-202), or blocked or in review in another worktree
    (T-204), is decided as it is there. Raises Invalid when it is to do already."""
    view = next((v for v in task_views(project, Filters()) if v["task"].get("id") == task_id), None)
    if view is None:
        show_tasks(project, [task_id])  # NotFound when it doesn't exist; otherwise it is archived (finished)
        kind = "done"
    else:
        kind = view["derived"].get("kind")
    if kind == "todo":
        store = project.store.read_claims()
        record = store["waiting"].get(task_id)
        if task_id in store["claims"]:
            kind = "active"  # the claim's holder check says whose it is
        elif isinstance(record, dict) and record.get("worktree") != project.store.working_copy():
            kind = kind_of(vocabulary({"meta": project.store.get_settings()}), record.get("status"))
    commands = {
        "active": "fail" if failing else "release",
        "review": "reject",
        "blocked": "unblock",
        "done": "reopen",
        "skipped": "reopen",
    }
    if kind not in commands:
        raise Invalid(f"{task_id} is already {kind}")
    return commands[kind]


def claim_record(work, data, task_id, agent):
    """A new claim on the task, from this working copy (its worktree and branch, T-202)."""
    here = work.here()
    return {
        "task": task_id,
        "agent": agent,
        "branch": here["branch"],
        "worktree": here["worktree"],
        "claimed_at": now_iso(),
        "lease_expires_at": lease_until(data),
    }


def completion_record(work, task_id, agent):
    """Done in this working copy: so `next` doesn't hand the task out again in another before the branches merge."""
    here = work.here()
    return {"task": task_id, "agent": agent, "branch": here["branch"], "worktree": here["worktree"], "completed_at": now_iso()}


def waiting_entry(work, task):
    """The shared record of a task blocked or sent for review in this working copy (T-204)."""
    here = work.here()
    return {
        "task": task["id"],
        "status": task.get("status"),
        "waiting_on": task.get("waiting_on"),
        "questions": list(task.get("questions") or []),
        "worktree": here["worktree"],
        "branch": here["branch"],
        "at": now_iso(),
    }


def adopt_waiting(task, data, store, here, command):
    """When `task` waits in another worktree (blocked or in review there, per the shared store) but not in this one,
    and `command` decides it: give this copy the waiting state, so the command applies here as it would there. Returns
    the worktree it waits in (where the decision is applied too, by apply_resolutions), or None."""
    record = store["waiting"].get(task["id"])
    if command not in REMOTE_DECISIONS or not isinstance(record, dict) or record.get("worktree") == here:
        return None
    vocab = vocabulary(data)
    if kind_of(vocab, task.get("status")) in REMOTE_DECISIONS[command]:
        return None  # it waits here too: decided here as usual
    if kind_of(vocab, record.get("status")) not in REMOTE_DECISIONS[command]:
        return None
    task["status"] = record["status"]
    if record.get("waiting_on"):
        task["waiting_on"] = record["waiting_on"]
    if record.get("questions"):
        task["questions"] = list(record["questions"])
    return record.get("worktree")


def newly_ready(data, task_id, archived):
    """The to-do tasks waiting on `task_id` whose dependencies are now all done."""
    ready = {t.get("id") for t in ready_tasks(data, archived)}
    return [t.get("id") for t in data["tasks"] if task_id in (t.get("dependencies") or []) and t.get("id") in ready]


def change_task_status(project, command, task_id, **options):
    """One status command on a task, as `project.actor`. `options` are the command's: status, note, reason, commit,
    on, questions, answers, resolution, duplicate_of. Returns the task after the change and what goes with it:
    `previous` (its status before), `claim` (its claim now), `unblocked` (after done), `hint` (the archive hint, at
    most once a day), `stranded` (reopened in a closed sprint, T-330) and `expired` (the claims whose lease ran out)."""
    agent = project.actor
    status = options.get("status")
    answers = list(options.get("answers") or [])

    def change(work):
        data, store, archived = work.document, work.claims(), archived_in(work)
        claims = store["claims"]
        here = work.here()["worktree"]
        expired = drop_expired_claims(data, claims)
        apply_resolutions(here, data, store, archived)
        task = find_task(data, task_id, archived)
        if command in OWN_LEASE_COMMANDS:
            own = own_lease_expired(expired, data, task_id, agent, f"atm task state {task_id} {COMMAND_STATES[command]}")
            if own:
                work.defer(own)  # the expiry is written first, so `atm task details` agrees with the error
                return {"task": task, "expired": expired}
        check_holder(claims, task_id, agent, command)
        duplicate_of = options.get("duplicate_of")
        if duplicate_of and not any(t.get("id") == duplicate_of for t in data["tasks"]) and not archived(duplicate_of):
            raise Invalid(f"{task_id} can't be a duplicate of {duplicate_of}, which doesn't exist")
        waits_in = adopt_waiting(task, data, store, here, command)
        previous = change_status(
            task,
            data,
            command,
            reason=options.get("reason"),
            agent=agent,
            status=status,
            note=options.get("note"),
            commit=options.get("commit"),
            waiting_on=options.get("on"),
            questions=list(options.get("questions") or []),
            answers=answers,
            resolution=options.get("resolution"),
            duplicate_of=duplicate_of,
            archived=archived,
        )
        if command == "start":
            claims[task_id] = claim_record(work, data, task_id, agent)
            store["completed"].pop(task_id, None)  # starting a task again on purpose
        elif command == "reopen":
            claims.pop(task_id, None)
            store["completed"].pop(task_id, None)  # not done any more, in any worktree
        else:
            claims.pop(task_id, None)
        if command == "done":
            store["completed"][task_id] = completion_record(work, task_id, agent)
        # Blocked or in review: recorded for every worktree (T-204); any other change ends the wait
        if command in ("block", "review"):
            store["waiting"][task_id] = waiting_entry(work, task)
        else:
            store["waiting"].pop(task_id, None)
            resolution = store["resolved"].pop(task_id, None)
            if waits_in:
                store["resolved"][task_id] = {
                    "task": task_id,
                    "command": command,
                    "answers": answers,
                    "reason": options.get("reason"),
                    "by": agent,
                    "at": now_iso(),
                    "worktree": waits_in,
                }
            elif resolution and resolution.get("worktree") != here:
                store["resolved"][task_id] = resolution  # still to be applied where it waits
        unblocked = newly_ready(data, task_id, archived) if command == "done" else []
        # The archive hint after done, at most once a day per clone (CLI-5.2), so a loop doesn't repeat it
        hint = archive_hint_for(data, work.size()) if command == "done" else None
        today = now_iso()[:10]
        if hint and store["hints"].get("archive") == today:
            hint = None
        if hint:
            store["hints"]["archive"] = today
        # Reopened in a closed sprint, the task is open where the loop's default selection never looks (T-330)
        stranded = None
        if command == "reopen":
            sprint = find_sprint(vocabulary(data), task.get("sprint"))
            if sprint and sprint["state"] == "closed":
                stranded = f"{task['id']} is now open in {move_out_hint(task['id'], sprint['id'])}"
        from_archive = all(t is not task for t in data["tasks"])
        return {
            "task": task,
            "previous": previous,
            "claim": claims.get(task_id),
            "unblocked": unblocked,
            "hint": hint,
            "stranded": stranded,
            "expired": expired,
            # The task as `atm show` gives it (show_tasks), so a front end can show it without reading the project
            "view": view_task(task, from_archive, data["tasks"], archived, vocabulary(data)),
        }

    return project.store.change(change)


# One function per status change (§5.1, T-383), each with the parameters its command takes. `status` picks the status
# when the project has several of the target kind. Each returns a StatusChange (api.schema.json): the task after it,
# its status before (`previous`), its claim now, the tasks now ready (`unblocked`, after done), the archive hint (at
# most once a day), `stranded` (reopened in a closed sprint) and the claims that `expired` on the way.


def start_task(project, task_id, status=None):
    """To do -> in progress, claimed by `project.actor`; its dependencies must be done."""
    return change_task_status(project, "start", task_id, status=status)


def review_task(project, task_id, note=None, status=None):
    """Asks for a review (`note`: what the reviewer should check); the claim is released."""
    return change_task_status(project, "review", task_id, note=note, status=status)


def done_task(project, task_id, note=None, commit=None, resolution=None, status=None):
    """Done (or waiting for the sprint review, by its review mode); the claim dropped."""
    return change_task_status(project, "done", task_id, note=note, commit=commit, resolution=resolution, status=status)


def reject_task(project, task_id, reason, status=None):
    """A task in review back to to do, for anyone."""
    return change_task_status(project, "reject", task_id, reason=reason, status=status)


def approve_task(project, task_id, note=None, status=None):
    """Approves a review; it stays in review until its agent takes it back."""
    return change_task_status(project, "approve", task_id, note=note, status=status)


def request_changes(project, task_id, reason, status=None):
    """Asks for changes; its agent gets it back first."""
    return change_task_status(project, "request-changes", task_id, reason=reason, status=status)


def block_task(project, task_id, on, questions, reason=None, status=None):
    """Blocked on a decision by `on` (user or agent), with its questions; the claim released."""
    return change_task_status(project, "block", task_id, on=on, questions=questions, reason=reason, status=status)


def unblock_task(project, task_id, answers, status=None):
    """Back to to do, with the answers (one per question) as a note."""
    return change_task_status(project, "unblock", task_id, answers=answers, status=status)


def reopen_task(project, task_id, reason, status=None):
    """A done or skipped task back to to do."""
    return change_task_status(project, "reopen", task_id, reason=reason, status=status)


def skip_task(project, task_id, reason, resolution=None, duplicate_of=None, status=None):
    """Not needed: skipped (a to-do or blocked task)."""
    return change_task_status(
        project, "skip", task_id, reason=reason, resolution=resolution, duplicate_of=duplicate_of, status=status
    )


def fail_task(project, task_id, reason=None, status=None):
    """In progress back to to do, with a failure note."""
    return change_task_status(project, "fail", task_id, reason=reason, status=status)


def release_task(project, task_id, note=None, status=None):
    """In progress back to to do, with a handoff note."""
    return change_task_status(project, "release", task_id, note=note, status=status)


# Each command's function, for a front end that takes the command's name (the CLI)
STATUS_FUNCTIONS = {
    "start": start_task,
    "review": review_task,
    "done": done_task,
    "reject": reject_task,
    "approve": approve_task,
    "request-changes": request_changes,
    "block": block_task,
    "unblock": unblock_task,
    "reopen": reopen_task,
    "skip": skip_task,
    "fail": fail_task,
    "release": release_task,
}


def add_task_note(project, task_id, text):
    """A note on a task, whatever its status (DM-4), at most 500 characters. Returns the task."""

    def change(work):
        task = find_task(work.document, task_id, archived_in(work))
        stamp = now_iso()
        add_note(task, "note", text, project.actor, stamp)
        touch(task, project.actor, stamp)
        return task

    return project.store.change(change)


# ==== lib/claims.py ====
# atm-lib: claims and the loop's next task (docs/layered-architecture.md §5.1): `atm next` (with `--claim`, atomic),
# heartbeats, the live claims. Each runs as one change of the store (§3.3); the loop's rule itself (`next_task`,
# `Selection`) is lib/loop.py's.

# heartbeat --auto renews a claim once its last renewal is this old (CLI-6.2)
AUTO_RENEW_AFTER = timedelta(minutes=10)


def selection(sprints=None, tasks=None, all_tasks=False, backlog=False, task_type=None):
    """What the loop works on: the active sprint by default, named sprints, a list of tasks, all, or the backlog."""
    return Selection(sprints=sprints, tasks=tasks, all_tasks=all_tasks, backlog=backlog, task_type=task_type)


def next_task_for(project, sel, with_user=False, claim=False, status=None):
    """The task the loop takes next for `project.actor` (the dashboard's Next up is the same rule); with `claim`,
    started and claimed in the same change, so no other agent can take it. Returns {task, claim, reason, expired}:
    `reason` says why there is none."""
    agent = project.actor

    def change(work):
        data, store, archived = work.document, work.claims(), archived_in(work)
        expired = drop_expired_claims(data, store["claims"])
        apply_resolutions(work.here()["worktree"], data, store, archived)
        task, reason = next_task(data, store, sel=sel, agent=agent, with_user=with_user, archived=archived)
        new = None
        if task and claim:
            change_status(task, data, "start", agent=agent, status=status, archived=archived)
            new = store["claims"][task["id"]] = claim_record(work, data, task["id"], agent)
        return {"task": task, "claim": new, "reason": reason, "expired": expired}

    return project.store.change(change)


def renew_lease(project, task_id, expire=True):
    """Extend the actor's lease on a task (`atm heartbeat`; `atm run` with `expire` false: it renews a live claim and
    lets nothing else expire). Returns {claim, minutes, expired}; raises Claimed when the task has no live claim (its
    lease expired) or another agent holds it."""
    agent = project.actor

    def change(work):
        data, claims = work.document, work.claims()["claims"]
        if not expire:
            claim = claims.get(task_id)
            try:
                alive = claim is not None and parse_iso(claim["lease_expires_at"]) > now()
            except (KeyError, TypeError, ValueError):
                alive = False
            if not alive:
                raise Claimed(f"{task_id} has no live claim")
            check_holder(claims, task_id, agent, "renew the claim on")
            claim["lease_expires_at"] = lease_until(data)
            claim["renewed_at"] = now_iso()
            return {"claim": claim, "minutes": lease_minutes(data), "expired": []}
        expired = drop_expired_claims(data, claims)
        claim = claims.get(task_id)
        if not claim:
            own = own_lease_expired(expired, data, task_id, agent, None)
            gone = any(c.get("task") == task_id for c in expired)
            work.defer(own or Claimed(f"{task_id} has no live claim" + (" (its lease expired)" if gone else "")))
            return None
        check_holder(claims, task_id, agent, "heartbeat")
        claim["lease_expires_at"] = lease_until(data)
        claim["renewed_at"] = now_iso()
        return {"claim": claim, "minutes": lease_minutes(data), "expired": expired}

    return project.store.change(change)


def renewed_at(claim):
    try:
        return parse_iso(claim.get("renewed_at") or claim["claimed_at"])
    except (KeyError, TypeError, ValueError):
        return None


def renew_due_claims(project):
    """`heartbeat --auto`: renew the actor's live claims whose last renewal is ten minutes old. Most calls only read
    the claims (no change, nothing written). Returns the claims renewed."""
    agent, current = project.actor, now()

    def due(claims):
        return [
            c
            for c in claims.values()
            if isinstance(c, dict)
            and c.get("agent") == agent
            and parse_iso(c["lease_expires_at"]) > current
            and (renewed_at(c) is None or current - renewed_at(c) >= AUTO_RENEW_AFTER)
        ]

    if not due(project.store.read_claims()["claims"]):
        return []

    def change(work):
        renewed = due(work.claims()["claims"])
        for claim in renewed:
            claim["lease_expires_at"] = lease_until(work.document)
            claim["renewed_at"] = now_iso()
        return renewed

    return project.store.change(change)


def expire_claims(project):
    """Drops the claims whose lease ran out (their tasks back to todo), and gives the live ones, by task (`atm
    claims`; `list_claims` only reads). Returns {claims, expired}."""

    def change(work):
        claims = work.claims()["claims"]
        expired = drop_expired_claims(work.document, claims)
        return {"claims": sorted(claims.values(), key=lambda c: c["task"]), "expired": expired}

    return project.store.change(change)


# ==== lib/edit.py ====
# atm-lib: adding and changing tasks (`atm add`, one task or many; `atm edit`) and checking the task data (`atm
# check`), docs/layered-architecture.md §5.1. IDs come from meta.next_id inside the store's change, so two agents adding
# at once never get the same one (docs/v4/data-model.md DM-2.3). Status and timestamps are not edited here: they change
# only through the status commands.

LABEL = re.compile(r"^[a-z0-9_-]{1,32}$")
DEFAULT_ID = ("T-", 3)


def id_format(data):
    """The project's ID pattern: the prefix and digit count of its last numbered task ID (T-001: "T-", 3)."""
    for task in reversed(data["tasks"]):
        parts = split_id(task.get("id"))
        if parts:
            return parts[0], len(parts[1])
    return DEFAULT_ID


def highest_number(data, prefix):
    numbers = [int(p[1]) for p in (split_id(t.get("id")) for t in data["tasks"]) if p and p[0] == prefix]
    return max(numbers, default=0)


def next_number(data):
    """meta.next_id, but never behind the highest ID in the file (tasks added by hand, or by an older atm)."""
    prefix, _ = id_format(data)
    stored = data.get("meta", {}).get("next_id") if isinstance(data.get("meta"), dict) else None
    floor = highest_number(data, prefix) + 1
    return max(stored, floor) if isinstance(stored, int) and not isinstance(stored, bool) else floor


def allocate_ids(data, count):
    """`count` new IDs in the project's pattern; advances meta.next_id."""
    prefix, width = id_format(data)
    first = next_number(data)
    ids = [prefix + str(n).zfill(width) for n in range(first, first + count)]
    if not isinstance(data.get("meta"), dict):
        data["meta"] = {}
    data["meta"]["next_id"] = first + count
    return ids


def check_labels(labels):
    for label in labels:
        if not isinstance(label, str) or not LABEL.match(label):
            raise AtmError(
                f"label {label!r}: use lowercase letters, digits, - and _ (at most 32 characters)", 2
            )
    return labels


def open_sprint(vocab, name):
    """The ID of the sprint called `name`, which a task can be put in: it must exist and not be closed (SR-2.5)."""
    sprint = find_sprint(vocab, name)
    if sprint is None:
        known = ", ".join(s["id"] for s in vocab["sprints"] if s["state"] != "closed") or "none yet: atm sprint add"
        raise AtmError(f"no sprint {name!r}; the open sprints: {known}", 2)
    if sprint["state"] == "closed":
        raise AtmError(f"the sprint {sprint['id']!r} is closed; a task can only go into a planned or active sprint", 2)
    return sprint["id"]


def check_with(value):
    """`with`: who the work is done together with; `user` for now (SR-6.5)."""
    if value != "user":
        raise AtmError(f"with {value!r}: use user (work done together with the user)", 2)
    return value


def check_review(value):
    if value not in REVIEW_MODES:
        raise AtmError(f"review {value!r}: use one of {', '.join(REVIEW_MODES)}", 2)
    return value


def check_related(task_id, related, known, archived):
    """`archived(id)`: the archived task with that ID, or None (the store's lookup, from atm-lib)."""
    for other in related:
        if other == task_id:
            raise AtmError(f"{task_id} can't be related to itself", 2)
        if other not in known and not archived(other):
            raise AtmError(f"{task_id} can't be related to {other}, which doesn't exist", 2)
    return related


def new_task(vocab, fields, actor, stamp):
    """A complete new task from the given fields, checked against the vocabularies."""
    # Tasks go into sprints (or the backlog): phases became sprints (SR-5.5, SR-8.3); a "phase" given is ignored
    for key in ("name", "type"):
        if fields.get(key) in (None, ""):
            raise AtmError(f"a new task needs a {key}", 2)
    types = [t["id"] for t in vocab["types"]]
    if fields["type"] not in types:
        raise AtmError(f"unknown type {fields['type']!r}; use one of {', '.join(types)}", 2)
    priorities = [p["id"] for p in vocab["priorities"]]
    if fields.get("priority") is not None and fields["priority"] not in priorities:
        raise AtmError(f"unknown priority {fields['priority']!r}; use one of {', '.join(priorities)}", 2)
    task = {"id": None, "name": str(fields["name"]), "type": fields["type"]}
    if fields.get("priority") is not None:
        task["priority"] = fields["priority"]
    labels = check_labels(list(fields.get("labels") or []))
    if labels:
        task["labels"] = labels
    task.update(
        {
            "description": str(fields.get("description") or ""),
            "requirements": list(fields.get("requirements") or []),
            "use_cases": list(fields.get("use_cases") or []),
            "status": status_for(vocab, "todo"),
            "dependencies": list(fields.get("dependencies") or []),
            "files": list(fields.get("files") or []),
            "created_at": stamp,
            "updated_at": stamp,
        }
    )
    if actor:
        task["created_by"] = actor
        task["updated_by"] = actor
    if fields.get("sprint"):
        set_sprint(task, open_sprint(vocab, fields["sprint"]), stamp)
    if fields.get("release"):
        task["release"] = known_release(vocab, fields["release"])
    # "auto" (--components auto): the components its files are in (SR-2.7)
    requested = fields.get("components") or []
    components = suggest_components(vocab, task["files"]) if requested == "auto" else component_ids(vocab, requested)
    if components:
        task["components"] = components
    if fields.get("review"):
        task["review"] = check_review(fields["review"])
    if fields.get("with"):
        task["with"] = check_with(fields["with"])
    if fields.get("related"):
        task["related"] = list(fields["related"])  # checked once the new IDs are known (add_tasks)
    return task


def find_cycle(tasks):
    """A dependency cycle among `tasks` (as a list of IDs), or None."""
    deps = {t["id"]: t.get("dependencies") or [] for t in tasks}
    done = set()
    # Depth-first with an explicit stack, not recursion: a chain of thousands of tasks (or a hostile file) can't hit
    # Python's recursion limit (docs/security-review.md F12). `path` is the current chain; `on_path` its positions.
    for start in deps:
        if start in done:
            continue
        path, on_path, stack = [start], {start: 0}, [iter(deps[start])]
        while stack:
            dep = next(stack[-1], None)
            if dep is None:
                node = path.pop()
                del on_path[node]
                done.add(node)
                stack.pop()
            elif dep in on_path:
                return path[on_path[dep] :] + [dep]
            elif dep in deps and dep not in done:
                on_path[dep] = len(path)
                path.append(dep)
                stack.append(iter(deps[dep]))
    return None


def add_tasks(data, entries, actor, archived):
    """Add `entries` (dicts of task fields; an "id" there is a placeholder other entries' dependencies may use) to
    `data`. Returns the new tasks. Raises AtmError, before changing anything, if one is invalid. `archived(id)` looks
    up an archived task (a dependency on one is fine)."""
    vocab = vocabulary(data)
    stamp = now_iso()
    tasks = [new_task(vocab, e, actor, stamp) for e in entries]
    placeholders = [e.get("id") for e in entries]
    for p in placeholders:
        if p is not None and placeholders.count(p) > 1:
            raise AtmError(f"placeholder ID {p!r} is used twice", 2)
    ids = allocate_ids(data, len(tasks))
    rename = {p: real for p, real in zip(placeholders, ids) if p is not None}
    existing = {t.get("id") for t in data["tasks"]}
    for task, real in zip(tasks, ids):
        task["id"] = real
        task["dependencies"] = [rename.get(d, d) for d in task["dependencies"]]
        # An archived task is done: depending on it is fine, as atm check says (T-356)
        missing = [d for d in task["dependencies"] if d not in existing and d not in ids and not archived(d)]
        if missing:
            raise AtmError(f"{real} ({task['name']}) depends on {', '.join(missing)}, which doesn't exist", 2)
        if task.get("related"):
            related = [rename.get(r, r) for r in task["related"]]
            task["related"] = check_related(real, related, existing | set(ids), archived)
    cycle = find_cycle(data["tasks"] + tasks)
    if cycle:
        raise AtmError("these dependencies would make a cycle: " + " -> ".join(cycle), 2)
    data["tasks"].extend(tasks)
    return tasks


def add_new_tasks(project, entries):
    """`atm add`: the tasks `entries` describe (dicts of task fields; an "id" there is a placeholder the others'
    dependencies may use), with the next IDs, all or none, as `project.actor`. Returns {tasks, vocab}."""

    def change(work):
        tasks = add_tasks(work.document, entries, project.actor, archived_in(work))
        return {"tasks": tasks, "vocab": vocabulary(work.document)}

    return project.store.change(change)


class TaskEdit:
    """What edit_task applies, as attributes (name, description, sprint, backlog, type, priority, release, review,
    with_, components; add_/remove_ file, dep, label, component, related, requirement, use_case); what isn't given
    changes nothing."""

    def __init__(self, **given):
        self.__dict__.update(given)

    def __getattr__(self, name):
        return [] if name.startswith(("add_", "remove_")) else None


def edit_existing_task(
    project,
    task_id,
    *,
    name=None,
    description=None,
    sprint=None,
    backlog=False,
    task_type=None,
    priority=None,
    release=None,
    review=None,
    with_=None,
    components=None,
    add_files=(),
    remove_files=(),
    add_dependencies=(),
    remove_dependencies=(),
    add_labels=(),
    remove_labels=(),
    add_components=(),
    remove_components=(),
    add_related=(),
    remove_related=(),
    add_requirements=(),
    remove_requirements=(),
    add_use_cases=(),
    remove_use_cases=(),
):
    """`atm edit`: change a task's definition (never its status), as `project.actor`. None (or an empty list) leaves
    a field as it is; an empty `release`, `review` or `with_` removes it; `backlog` takes it out of its sprint;
    `components="auto"` adds the components its files are in. Returns {task, changed, vocab}: `changed` the names of
    the fields that changed (nothing is written when none did)."""
    changes = TaskEdit(
        name=name,
        description=description,
        sprint=sprint,
        backlog=backlog,
        type=task_type,
        priority=priority,
        release=release,
        review=review,
        with_=with_,
        components=components,
        add_file=list(add_files),
        remove_file=list(remove_files),
        add_dep=list(add_dependencies),
        remove_dep=list(remove_dependencies),
        add_label=list(add_labels),
        remove_label=list(remove_labels),
        add_component=list(add_components),
        remove_component=list(remove_components),
        add_related=list(add_related),
        remove_related=list(remove_related),
        add_requirement=list(add_requirements),
        remove_requirement=list(remove_requirements),
        add_use_case=list(add_use_cases),
        remove_use_case=list(remove_use_cases),
    )

    def change(work):
        archived = archived_in(work)
        task = find_task(work.document, task_id, archived)
        changed = edit_task(work.document, task, changes, archived)
        if changed:
            touch(task, project.actor, now_iso())
        return {"task": task, "changed": changed, "vocab": vocabulary(work.document)}

    return project.store.change(change)


def edit_task(data, task, args, archived):
    """Apply atm edit's options to `task`; returns the names of the fields that changed. Raises AtmError (nothing
    changed yet in the file) when a value is invalid. A description from a file is the caller's to read."""
    vocab = vocabulary(data)
    changed = []

    def set_field(key, value):
        if task.get(key) != value:
            task[key] = value
            changed.append(key)

    def edit_list(key, add, remove):
        current = list(task.get(key) or [])
        updated = [v for v in current if v not in remove] + [v for v in add if v not in current]
        if updated != current:
            task[key] = updated
            changed.append(key)

    if args.name is not None:
        set_field("name", args.name)
    if args.description is not None:
        set_field("description", args.description)
    if args.sprint is not None or args.backlog:
        if set_sprint(task, None if args.backlog else open_sprint(vocab, args.sprint), now_iso()):
            changed.append("sprint")
    if args.type is not None:
        types = [t["id"] for t in vocab["types"]]
        if args.type not in types:
            raise AtmError(f"unknown type {args.type!r}; use one of {', '.join(types)}", 2)
        set_field("type", args.type)
    if args.priority is not None:
        priorities = [p["id"] for p in vocab["priorities"]]
        if args.priority not in priorities:
            raise AtmError(f"unknown priority {args.priority!r}; use one of {', '.join(priorities)}", 2)
        set_field("priority", args.priority)
    edit_list("files", args.add_file, args.remove_file)
    edit_list("labels", check_labels(args.add_label), args.remove_label)
    # An entry is an ID or a Markdown link to where it is defined (T-391): any text but an empty one
    for key, add, remove in (
        ("requirements", args.add_requirement, args.remove_requirement),
        ("use_cases", args.add_use_case, args.remove_use_case),
    ):
        if any(not isinstance(v, str) or not v.strip() for v in add):
            raise AtmError(f"an empty {key[:-1].replace('_', ' ')} can't be added", 2)
        edit_list(key, add, remove)
    for key, value, check in (
        ("release", args.release, lambda v: known_release(vocab, v)),
        ("review", args.review, check_review),
        ("with", args.with_, check_with),
    ):
        if value == "":
            if key in task:
                del task[key]
                changed.append(key)
        elif value is not None:
            set_field(key, check(value))
    if args.components == "auto":
        edit_list("components", suggest_components(vocab, task.get("files") or []), [])
    edit_list("components", component_ids(vocab, args.add_component), args.remove_component)
    known = {t.get("id") for t in data["tasks"]}
    edit_list("related", check_related(task["id"], args.add_related, known, archived), args.remove_related)
    for key in ("components", "related"):
        if key in task and not task[key]:
            del task[key]
    ids = {t.get("id") for t in data["tasks"]}
    missing = [d for d in args.add_dep if d not in ids and not archived(d)]
    if missing:
        raise AtmError(f"{task['id']} can't depend on {', '.join(missing)}, which doesn't exist", 2)
    if task["id"] in args.add_dep:
        raise AtmError(f"{task['id']} can't depend on itself", 2)
    edit_list("dependencies", args.add_dep, args.remove_dep)
    cycle = find_cycle(data["tasks"])
    if cycle:
        raise AtmError("these dependencies would make a cycle: " + " -> ".join(cycle), 2)
    return changed


def check_resolution(vocab, task, known_ids, errors):
    """A task's resolution, duplicate_of and related (SR-2.8): a resolution only on a done or skipped task, one of
    RESOLUTIONS; duplicate_of only with the duplicate resolution, naming another task that exists (in either file)."""
    task_id = task["id"]
    resolution = task.get("resolution")
    if "resolution" in task:
        if resolution not in RESOLUTIONS:
            errors.append(f"{task_id}: the resolution {resolution!r} is not one of {', '.join(RESOLUTIONS)}")
        elif kind_of(vocab, task.get("status")) not in ("done", "skipped"):
            errors.append(f"{task_id}: has a resolution ({resolution}) but isn't done or skipped")
    if "duplicate_of" in task:
        other = task["duplicate_of"]
        if resolution != "duplicate":
            errors.append(f"{task_id}: duplicate_of needs the resolution duplicate")
        if other == task_id or other not in known_ids:
            errors.append(f"{task_id}: duplicate_of {other!r} isn't another task")
    related = task.get("related", [])
    if not (isinstance(related, list) and all(isinstance(r, str) for r in related)):
        errors.append(f"{task_id}: 'related' must be a list of task IDs")
    else:
        for other in related:
            if other == task_id or other not in known_ids:
                errors.append(f"{task_id}: related to {other}, which isn't another task")


def check_data(data, root, files=False, archived=()):
    """Problems in a task file: (errors, warnings), each a list of messages. `archived` are the archive's tasks:
    dependencies on them are fine, and an ID in both files is an error. With `files`, also a warning for each file a
    finished task lists that doesn't exist (often just history: files get renamed and deleted later)."""
    errors, warnings = [], []
    archived_ids = {t.get("id") for t in archived if isinstance(t, dict)}
    try:
        vocab = vocabulary(data)
    except AtmError as e:
        errors.append(str(e))
        vocab = vocabulary({})
    tasks = data.get("tasks", [])
    ids = [t.get("id") for t in tasks if isinstance(t, dict)]
    for task_id in sorted({i for i in ids if ids.count(i) > 1}, key=str):
        errors.append(f"{task_id}: the ID is used by {ids.count(task_id)} tasks")
    for task_id in sorted(set(ids) & archived_ids, key=str):
        errors.append(f"{task_id}: in both tasks.json and the archive (atm db archive keeps the tasks.json copy)")
    known_ids = set(ids) | archived_ids
    statuses = {s["id"] for s in vocab["statuses"]}
    types = {t["id"] for t in vocab["types"]}
    priorities = {p["id"] for p in vocab["priorities"]}
    for i, task in enumerate(tasks):
        if not isinstance(task, dict) or not isinstance(task.get("id"), str) or not task["id"]:
            errors.append(f"task #{i + 1}: needs an \"id\" (a non-empty string)")
            continue
        task_id = task["id"]
        if not valid_task_id(task_id):
            errors.append(f"{task_id!r}: an ID is letters, digits, '.', '_' and '-', at most 64, not starting with '-'")
        for key, kind in (("name", str), ("type", str), ("status", str)):
            if not isinstance(task.get(key), kind):
                errors.append(f"{task_id}: needs a {key!r} (a string)")
        if isinstance(task.get("status"), str) and kind_of(vocab, task["status"]) is None:
            errors.append(f"{task_id}: the status {task['status']!r} is not one of the project's statuses")
        if isinstance(task.get("type"), str) and task["type"] not in types:
            warnings.append(f"{task_id}: the type {task['type']!r} is not one of the project's types")
        if "priority" in task and task["priority"] not in priorities:
            warnings.append(f"{task_id}: the priority {task['priority']!r} is not one of the project's priorities")
        for key in ("dependencies", "files", "labels", "requirements", "use_cases"):
            if key in task and not (isinstance(task[key], list) and all(isinstance(v, str) for v in task[key])):
                errors.append(f"{task_id}: {key!r} must be a list of strings")
        if "sprint" in task and find_sprint(vocab, task["sprint"]) is None:
            errors.append(f"{task_id}: the sprint {task['sprint']!r} is not one of meta.sprints")
        if "release" in task and find_release(vocab, task["release"]) is None:
            errors.append(f"{task_id}: the release {task['release']!r} is not one of meta.releases")
        if "sprint_history" in task and not (
            isinstance(task["sprint_history"], list)
            and all(isinstance(e, dict) and isinstance(e.get("sprint"), str) for e in task["sprint_history"])
        ):
            errors.append(f'{task_id}: "sprint_history" must be a list of {{"sprint", "from", "to"}}')
        task_files = [f for f in task.get("files") or [] if isinstance(f, str)]
        if vocab["components"] and task_files and not suggest_components(vocab, task_files):
            warnings.append(f"{task_id}: its files match no component (meta.components' paths)")
        check_resolution(vocab, task, known_ids, errors)
        for dep in task.get("dependencies") or []:
            if isinstance(dep, str) and dep not in known_ids:
                errors.append(f"{task_id}: depends on {dep}, which doesn't exist")
        for label in task.get("labels") or []:
            if isinstance(label, str) and not LABEL.match(label):
                errors.append(f"{task_id}: the label {label!r} isn't lowercase letters, digits, - and _")
        # Tasks list the files they will create, so only a finished task's missing files are worth a word
        if files and kind_of(vocab, task.get("status")) == "done":
            for f in task.get("files") or []:
                if isinstance(f, str) and not os.path.exists(os.path.join(root, f)):
                    warnings.append(f"{task_id}: the file {f} doesn't exist")
    for t in open_in_closed_sprints(vocab, [t for t in tasks if isinstance(t, dict) and isinstance(t.get("id"), str)]):
        warnings.append(f"{t['id']}: {t['status']} in {move_out_hint(t['id'], t['sprint'])}")
    for sprint in vocab["sprints"]:
        if sprint.get("release") and find_release(vocab, sprint["release"]) is None:
            errors.append(f"the sprint {sprint['id']!r}: the release {sprint['release']!r} is not one of meta.releases")
    well_formed = [t for t in tasks if isinstance(t, dict) and isinstance(t.get("dependencies", []), list)]
    cycle = find_cycle([{"id": t.get("id"), "dependencies": t.get("dependencies") or []} for t in well_formed])
    if cycle:
        errors.append("dependency cycle: " + " -> ".join(str(c) for c in cycle))
    return errors, warnings


def check_project(project, files=False):
    """`atm check`: what is wrong with the task data as stored. Returns {errors, warnings, count}: `count` the tasks,
    None when the data isn't task data at all. With `files`, a finished task's files that don't exist are warned about
    too (relative to the working copy)."""
    raw = project.store.raw()
    data = raw["document"]
    if data is None:
        return {"errors": ["tasks.json is not a task file (expected an object with a 'tasks' list)"], "warnings": [], "count": None}
    errors_before = [raw["archive_error"]] if raw["archive_error"] else []
    errors, warnings = check_data(data, project.store.working_copy(), files=files, archived=raw["archive"])
    return {"errors": errors_before + errors, "warnings": warnings, "count": len(data["tasks"])}


# ==== lib/components.py ====
# atm-lib: the project's components (docs/sprints-and-releases-design.md §2, SR-2.7; docs/layered-architecture.md
# §5.1): list them, define one, change one; moving a label onto a component, or giving it to the tasks its globs match
# (T-324), changes the archive's tasks too, in the same store change. The rules (which files a component has, the IDs a
# task may name) are common's (common/components.py).

def _tasks(*documents):
    return [t for data in documents for t in data["tasks"] if isinstance(t, dict)]


def list_components(project):
    """meta.components in order, each with how many tasks have it, archived ones too."""
    store = project.store
    tasks = [t for t in store.list_tasks() if isinstance(t, dict)] + store.list_archived()
    return [
        {**c, "tasks": sum(1 for t in tasks if c["id"] in (t.get("components") or []))}
        for c in store.get_vocabularies()["components"]
    ]


def _label_entry(entry):
    """A meta.labels entry's ID: the entry itself when it is a string, else its "id"."""
    return entry if isinstance(entry, str) else entry.get("id") if isinstance(entry, dict) else None


def _label_text(meta, label):
    """The text meta.labels gives `label` (e.g. "Dashboard" for dashboard), or None."""
    entries = meta.get("labels") if isinstance(meta.get("labels"), list) else []
    entry = next((e for e in entries if _label_entry(e) == label), None)
    return entry["label"] if isinstance(entry, dict) and isinstance(entry.get("label"), str) else None


def _check_options(globs, review):
    if any(not glob for glob in globs):
        raise Invalid("--path needs a glob, e.g. 'dashboard/**'")
    if review and review not in REVIEW_MODES:
        raise Invalid(f"--review {review!r}: use one of {', '.join(REVIEW_MODES)}")


def _move_and_assign(data, archive_data, component, label, assign):
    """On every task of both files: `label` (if given) becomes the component, and with `assign` a task whose files
    the component's globs match gets it too; `label` leaves meta.labels. Returns the task IDs from the label and those
    by their files. A label no task has and meta.labels doesn't define (most likely a typo) is refused before anything
    changes."""
    meta = data["meta"]
    known_label = any(_label_entry(e) == label for e in meta.get("labels") or [])
    if label and not known_label and not any(label in (t.get("labels") or []) for t in _tasks(data, archive_data)):
        raise Invalid(f"no task has the label {label!r}")
    from_label, by_files = [], []
    for task in _tasks(data, archive_data):
        labels = task.get("labels") if isinstance(task.get("labels"), list) else []
        moved = bool(label) and label in labels
        current = task.get("components") if isinstance(task.get("components"), list) else []
        matched = (
            assign
            and component["id"] not in current
            and any(matches(component, f) for f in task.get("files") or [] if isinstance(f, str))
        )
        if not (moved or matched):
            continue
        if moved:
            task["labels"] = [x for x in labels if x != label]
            if not task["labels"]:
                del task["labels"]
        if component["id"] not in current:
            task["components"] = current + [component["id"]]
        (from_label if moved else by_files).append(task["id"])
    if label and isinstance(meta.get("labels"), list):
        meta["labels"] = [e for e in meta["labels"] if _label_entry(e) != label]
        if not meta["labels"]:
            del meta["labels"]
    return from_label, by_files


def add_component(project, component_id, label=None, paths=(), review=None, from_label=None, assign=False):
    """Define a component; `from_label` moves that label onto it on every task (both files) and drops it from
    meta.labels; `assign` gives it to every task whose files its globs match. One change. Returns {component,
    from_label, by_files}."""
    if not RELEASE_ID.fullmatch(component_id):
        raise Invalid(f"{component_id!r}: a component ID is letters, digits, '.', '-' and '_' (at most 64)")
    globs = list(paths or [])
    _check_options(globs, review)
    if assign and not globs:
        raise Invalid("--assign gives the component to the tasks its globs match: it needs --path")

    def change(work):
        data = work.document
        if any(c["id"] == component_id for c in vocabulary(data)["components"]):
            raise Invalid(f"there is already a component {component_id!r} (atm component edit changes it)")
        meta = data.setdefault("meta", {})
        component = {"id": component_id}
        # The name shown: the label given, else the label's own text (meta.labels), else none
        label_text = label or (_label_text(meta, from_label) if from_label else None)
        if label_text:
            component["label"] = label_text
        component["paths"] = globs
        if review:
            component["review"] = review
        meta["components"] = list(meta.get("components") or []) + [component]
        moved, by_files = _move_and_assign(data, work.archive(), component, from_label, assign)
        vocabulary(data)  # checks meta.components, as atm init would
        return {"component": component, "from_label": moved, "by_files": by_files}

    return project.store.change(change)


def edit_component(project, component_id, label=None, add_paths=(), review=None, from_label=None, assign=False):
    """Change a component: `label` and `review` (empty: remove them), `add_paths`; `from_label` and `assign` as for
    add, the globs added included. One change. Returns {component, changed, from_label, by_files}."""
    globs = list(add_paths or [])
    _check_options(globs, review)

    def change(work):
        data = work.document
        known = [c["id"] for c in vocabulary(data)["components"]]
        if component_id not in known:
            raise NotFound(f"no component {component_id!r}; the components: {', '.join(known) or 'none yet'}", 2)
        component = next(c for c in data["meta"]["components"] if c.get("id") == component_id)
        changed = []
        for key, value in (("label", label), ("review", review)):
            if value == "" and key in component:
                del component[key]
                changed.append(key)
            elif value and component.get(key) != value:
                component[key] = value
                changed.append(key)
        added = [g for g in dict.fromkeys(globs) if g not in (component.get("paths") or [])]
        if added:
            component["paths"] = list(component.get("paths") or []) + added
            changed.append("paths")
        if assign and not component.get("paths"):
            raise Invalid(f"--assign gives {component_id} to the tasks its globs match, and it has none: add --add-path")
        moved, by_files = _move_and_assign(data, work.archive(), component, from_label, assign)
        vocabulary(data)  # checks meta.components, as atm init would
        return {"component": component, "changed": changed, "from_label": moved, "by_files": by_files}

    return project.store.change(change)


# ==== lib/sprints.py ====
# atm-lib: sprints and the backlog (docs/sprints-and-releases-design.md §3.1; docs/layered-architecture.md §5.1):
# list, add, edit, start, close and delete sprints (meta.sprints), move tasks between sprints and the backlog, list the
# backlog. Each write is one store change; a task's sprint changes only through set_sprint, which keeps its history.
# The functions take keyword parameters (T-383).

def sprint_overview(data, archived, include_closed):
    """The sprints in order, each with its task counts by kind (tasks.json and the archive): {..., tasks, by_kind}."""
    vocab = vocabulary(data)
    counts = {s["id"]: {kind: 0 for kind in KINDS} for s in vocab["sprints"]}
    pending = {s["id"]: 0 for s in vocab["sprints"]}
    for task in list(data["tasks"]) + list(archived):
        sprint = find_sprint(vocab, task.get("sprint")) if isinstance(task, dict) else None
        kind = kind_of(vocab, task.get("status")) if sprint else None
        if kind in KINDS:
            counts[sprint["id"]][kind] += 1
        if sprint and task.get("sprint_review") == "pending":
            pending[sprint["id"]] += 1
    return [
        {
            **s,
            "tasks": sum(counts[s["id"]].values()),
            "by_kind": {kind: n for kind, n in counts[s["id"]].items() if n},
            "review_pending": pending[s["id"]],
        }
        for s in vocab["sprints"]
        if include_closed or s["state"] != "closed"
    ]


def taken_names(data, archived):
    """Every sprint name in use: the sprints, and the sprints tasks in either file name (compared case-insensitively
    by the caller)."""
    names = [s.get("id") for s in data.get("meta", {}).get("sprints") or [] if isinstance(s, dict)]
    for task in list(data["tasks"]) + list(archived):
        if isinstance(task, dict):
            names.append(task.get("sprint"))
            names.extend(e.get("sprint") for e in task.get("sprint_history") or [] if isinstance(e, dict))
    return [n for n in names if isinstance(n, str)]


def check_due(due):
    if due and not DATE.fullmatch(due):
        raise Invalid(f"--due {due!r}: use a date, YYYY-MM-DD")
    return due


def apply_fields(sprint, vocab, goal, release, due):
    """goal, release and due on a sprint (None leaves one as it is); an empty value removes the field; a release must
    be one of meta.releases (SR-2.2). Returns the names changed."""
    changed = []
    release = known_release(vocab, release) if release else release
    for key, value in (("goal", goal), ("release", release), ("due", check_due(due))):
        if value is None:
            continue
        if value == "":
            if key in sprint:
                del sprint[key]
                changed.append(key)
        elif sprint.get(key) != value:
            sprint[key] = value
            changed.append(key)
    return changed


def in_backlog(vocab, task, finished=False):
    """A task with no sprint; a finished one only with `finished`."""
    return not task.get("sprint") and (finished or kind_of(vocab, task.get("status")) not in ("done", "skipped"))


def list_sprints(project, include_closed=False):
    """The sprints in order with their task counts (the closed ones too with `include_closed`), and how many tasks
    are in the backlog. Returns {sprints, backlog, has_sprints}."""
    store = project.store
    data = store.document()
    vocab = vocabulary(data)
    return {
        "sprints": sprint_overview(data, store.list_archived(), include_closed),
        "backlog": len([t for t in data["tasks"] if in_backlog(vocab, t)]),
        "has_sprints": bool(vocab["sprints"]),
    }


def backlog_tasks(project, finished=False):
    """The tasks with no sprint (the finished ones too with `finished`). Returns {tasks, vocab}."""
    data = project.store.document()
    vocab = vocabulary(data)
    return {"tasks": [t for t in data["tasks"] if in_backlog(vocab, t, finished=finished)], "vocab": vocab}


def add_sprint(project, name=None, goal=None, release=None, due=None, after=None):
    """A planned sprint, named `name` (generated without one), after the sprint `after` (default: last), with its
    goal, release and due date. Returns the sprint."""

    def change(work):
        data = work.document
        vocab = vocabulary(data)
        sprint_name = (name or "").strip() or generate_sprint_name(
            vocab["sprint_names"], taken_names(data, work.archive()["tasks"])
        )
        if find_sprint(vocab, sprint_name):
            raise Invalid(f"there is already a sprint {find_sprint(vocab, sprint_name)['id']!r}")
        if len(sprint_name) > MAX_SPRINT_NAME:
            raise Invalid(f"a sprint name is at most {MAX_SPRINT_NAME} characters")
        sprint = {"id": sprint_name, "state": "planned"}
        apply_fields(sprint, vocab, goal, release, due)
        meta = data.setdefault("meta", {})
        sprints = list(meta.get("sprints") or [])
        position = len(sprints)
        if after:
            previous = find_sprint(vocab, after)
            if previous is None:
                raise NotFound(f"no sprint {after!r} to add it after", 2)
            position = next(i for i, s in enumerate(sprints) if s.get("id") == previous["id"]) + 1
        sprints.insert(position, sprint)
        meta["sprints"] = sprints
        vocabulary(data)
        return sprint

    return project.store.change(change)


def stored_sprint(data, name):
    """The sprint called `name` as stored in meta.sprints (to change it), or an error naming the sprints."""
    vocab = vocabulary(data)
    found = find_sprint(vocab, name)
    if found is None:
        known = ", ".join(s["id"] for s in vocab["sprints"]) or "none yet: atm sprint add"
        raise NotFound(f"no sprint {name!r}; the sprints: {known}", 2)
    return next(s for s in data["meta"]["sprints"] if s.get("id") == found["id"])


def edit_sprint(project, name, goal=None, release=None, due=None):
    """Change a sprint's goal, release or due date (None leaves it, an empty value removes it). Returns {sprint,
    changed}."""

    def change(work):
        sprint = stored_sprint(work.document, name)
        changed = apply_fields(sprint, vocabulary(work.document), goal, release, due)
        if changed:
            vocabulary(work.document)
        return {"sprint": sprint, "changed": changed}

    return project.store.change(change)


def start_sprint(project, name):
    """Make a planned sprint the active one (one at a time). Returns the sprint."""

    def change(work):
        data = work.document
        sprint = stored_sprint(data, name)
        state = sprint.get("state", "planned")
        if state == "closed":
            raise Invalid(f"the sprint {sprint['id']!r} is closed; closed sprints aren't started again")
        if state == "active":
            raise Invalid(f"the sprint {sprint['id']!r} is already active")
        active = [s["id"] for s in vocabulary(data)["sprints"] if s["state"] == "active"]
        if active:
            raise Invalid(f"the sprint {active[0]!r} is active; close it first (one active sprint at a time)")
        sprint["state"] = "active"
        sprint["start"] = now_iso()
        return sprint

    return project.store.change(change)


def in_sprint(task, sprint):
    """Whether the task is in the sprint (names compare case-insensitively)."""
    return (
        isinstance(task, dict)
        and isinstance(task.get("sprint"), str)
        and task["sprint"].casefold() == sprint["id"].casefold()
    )


def added_mid_sprint(task, sprint):
    """Whether the task joined the sprint after it started: its history entry for the sprint (the last one) begins
    after the sprint's start (SR-2.5)."""
    entries = [
        e
        for e in task.get("sprint_history") or []
        if isinstance(e, dict) and isinstance(e.get("sprint"), str) and e["sprint"].casefold() == sprint["id"].casefold()
    ]
    start = sprint.get("start")
    return bool(entries and start and isinstance(entries[-1].get("from"), str) and entries[-1]["from"] > start)


def carry_target(data, vocab, sprint, to, new_name, taken):
    """Where `to` (carry_to) sends the unfinished tasks: a sprint ID, None for the backlog, or a new planned sprint
    (named `new_name`, added to meta.sprints after the one closing). Returns (target, created sprint or None)."""
    if new_name and to != "new":
        raise Invalid("--name names a new sprint: use it with --carry-to new")
    if to == "backlog":
        return None, None
    if to == "new":
        name = (new_name or "").strip() or generate_sprint_name(vocab["sprint_names"], taken)
        if find_sprint(vocab, name):
            raise Invalid(f"there is already a sprint {find_sprint(vocab, name)['id']!r}")
        if len(name) > MAX_SPRINT_NAME:
            raise Invalid(f"a sprint name is at most {MAX_SPRINT_NAME} characters")
        created = {"id": name, "state": "planned"}
        sprints = data["meta"]["sprints"]
        sprints.insert(next(i for i, s in enumerate(sprints) if s.get("id") == sprint["id"]) + 1, created)
        return name, created
    target = open_sprint(vocab, to)
    if target == sprint["id"]:
        raise Invalid(f"the tasks can't be carried to the sprint that is closing ({sprint['id']!r})")
    return target, None


def close_sprint(project, name, carry_to=None, new_name=None, reviewed=False):
    """Close a sprint (SR-4.3): its unfinished tasks go to `carry_to` (a sprint, `new`, named `new_name`, or
    `backlog`), keeping their status and history (SR-4.5); the sprint gets `end` and its report {done, carried, added,
    to}; with `reviewed` its batch tasks are marked reviewed (SR-4.4). Returns {sprint, created, target, was_active}."""
    actor = project.actor

    def change(work):
        data = work.document
        archived = work.archive()["tasks"]
        vocab = vocabulary(data)
        sprint = stored_sprint(data, name)
        if sprint.get("state") == "closed":
            raise Invalid(f"the sprint {sprint['id']!r} is already closed")
        own = [t for t in data["tasks"] if in_sprint(t, sprint)]
        own_archived = [t for t in archived if in_sprint(t, sprint)]

        def kind(task):
            return kind_of(vocab, task.get("status"))

        claimed = [t["id"] for t in own if kind(t) == "active"]
        if claimed:
            raise Invalid(
                f"{', '.join(claimed)} {'is' if len(claimed) == 1 else 'are'} in progress: finish or release "
                f"{'it' if len(claimed) == 1 else 'them'} before closing the sprint"
            )
        # Batch tasks wait for the end-of-sprint review (SR-4.4): /atm:close-sprint reviews them, then --reviewed
        pending = [t for t in own if t.get("sprint_review") == "pending"]
        if pending and not reviewed:
            ids = ", ".join(t["id"] for t in pending)
            raise Invalid(
                f"{ids} {'waits' if len(pending) == 1 else 'wait'} for the end-of-sprint review: review "
                f"{'it' if len(pending) == 1 else 'them'} with the user (/atm:close-sprint), then close with --reviewed"
            )
        unfinished = [t for t in own if kind(t) not in ("done", "skipped")]
        if unfinished and not carry_to:
            ids = ", ".join(t["id"] for t in unfinished)
            raise Invalid(
                f"{ids} {'is' if len(unfinished) == 1 else 'are'}n't finished: say where "
                f"{'it goes' if len(unfinished) == 1 else 'they go'} with --carry-to <sprint>, new or backlog"
            )
        target, created = (None, None)
        if unfinished:
            target, created = carry_target(data, vocab, sprint, carry_to, new_name, taken_names(data, archived))
        stamp = now_iso()
        report = {
            "done": [t["id"] for t in own + own_archived if kind(t) == "done"],
            "carried": [t["id"] for t in unfinished],
            "added": [t["id"] for t in own + own_archived if added_mid_sprint(t, sprint)],
        }
        if unfinished:
            report["to"] = target or "backlog"
        for task in unfinished:
            set_sprint(task, target, stamp)
            touch(task, actor, stamp)
        for task in pending:
            task["sprint_review"] = "reviewed"
            touch(task, actor, stamp)
        was_active = sprint.get("state") == "active"
        sprint["state"] = "closed"
        sprint["end"] = stamp
        sprint["report"] = report
        vocabulary(data)
        return {"sprint": sprint, "created": created, "target": target, "was_active": was_active}

    return project.store.change(change)


def mentions_sprint(task, sprint):
    """Whether the task's sprint history names the sprint (case-insensitively)."""
    return any(
        isinstance(e, dict) and isinstance(e.get("sprint"), str) and e["sprint"].casefold() == sprint["id"].casefold()
        for e in task.get("sprint_history") or []
    )


def delete_sprint(project, name, move_to=None, force=False):
    """Delete a sprint created by mistake (T-339). An empty one (no task in it, none ever) goes at once, whatever its
    state; an open one's unfinished tasks go to `move_to` (the backlog by default); one that tasks finished in or
    moved on from needs `force` when open, and a closed one with any history stays (its record and report).
    Returns {sprint, state, moved, target}."""
    actor = project.actor

    def change(work):
        data = work.document
        archived = work.archive()["tasks"]
        vocab = vocabulary(data)
        sprint = stored_sprint(data, name)
        state = sprint.get("state", "planned")

        def kind(task):
            return kind_of(vocab, task.get("status"))

        own = [t for t in data["tasks"] if in_sprint(t, sprint)]
        claimed = [t["id"] for t in own if kind(t) == "active"]
        if claimed:
            raise Invalid(
                f"{', '.join(claimed)} {'is' if len(claimed) == 1 else 'are'} in progress: finish or release "
                f"{'it' if len(claimed) == 1 else 'them'} before deleting the sprint"
            )
        unfinished = [t for t in own if kind(t) not in ("done", "skipped")]
        # Its history: tasks that finished in it (archived ones too), or were in it and moved on
        history = [t["id"] for t in own if t not in unfinished]
        history += [t.get("id") for t in archived if isinstance(t, dict) and in_sprint(t, sprint)]
        history += [
            t.get("id")
            for t in list(data["tasks"]) + list(archived)
            if isinstance(t, dict) and not in_sprint(t, sprint) and mentions_sprint(t, sprint)
        ]
        if history and state == "closed":
            raise Invalid(
                f"the sprint {sprint['id']!r} is closed and had tasks ({', '.join(history)}): its record and report "
                "stay; only an empty sprint can be deleted"
            )
        if history and not force:
            raise Invalid(
                f"tasks finished in the sprint {sprint['id']!r} or moved on from it ({', '.join(history)}): delete it "
                "with --force to drop that record (its tasks go to --move-to, the backlog by default)"
            )
        to = move_to or "backlog"
        target = None if to == "backlog" else open_sprint(vocab, to)
        if target is not None and target.casefold() == sprint["id"].casefold():
            raise Invalid(f"the tasks can't move to the sprint being deleted ({sprint['id']!r})")
        # With --force, the finished tasks still in it move too, so no task names a sprint that is gone
        moving = own if force else unfinished
        stamp = now_iso()
        for task in moving:
            set_sprint(task, target, stamp)
            touch(task, actor, stamp)
        data["meta"]["sprints"] = [s for s in data["meta"]["sprints"] if s is not sprint]
        vocabulary(data)
        return {"sprint": sprint, "state": state, "moved": [t["id"] for t in moving], "target": target}

    return project.store.change(change)


def move_target(vocab, task, to):
    """The sprint ID (or None, the backlog) `atm sprint move --to` means for `task`."""
    if to == "backlog":
        return None
    if to == "active":
        active = [s["id"] for s in vocab["sprints"] if s["state"] == "active"]
        if not active:
            raise Invalid("no sprint is active (atm sprint start <name>)")
        return active[0]
    if to == "next":
        # The first planned sprint after the task's own (for a task in the backlog or a closed sprint: the first
        # planned sprint)
        sprints = vocab["sprints"]
        current = find_sprint(vocab, task.get("sprint"))
        start = sprints.index(current) + 1 if current else 0
        following = [s["id"] for s in sprints[start:] if s["state"] == "planned"]
        if not following:
            raise Invalid(
                f"{task['id']}: no planned sprint after {current['id'] if current else 'the active one'}; "
                'add one with atm sprint add ["<name>"]'
            )
        return following[0]
    return open_sprint(vocab, to)


def move_tasks(project, ids, to):
    """Move to-do, blocked or in-review tasks into a sprint, the next planned one, the active one, or the backlog
    (claimed and finished tasks stay where they are), all or none. Returns [{id, from, to}]."""
    actor = project.actor

    def change(work):
        data = work.document
        vocab = vocabulary(data)
        plans, archived = [], archived_in(work)
        for task_id in ids:
            task = find_task(data, task_id, archived)
            kind = kind_of(vocab, task.get("status"))
            if kind == "active":
                raise Invalid(f"{task_id} is in progress (claimed): it stays where the work is happening")
            if kind in ("done", "skipped"):
                raise Invalid(f"{task_id} is {task.get('status')}: finished work stays in the sprint it was done in")
            plans.append((task, move_target(vocab, task, to)))
        stamp = now_iso()
        moved = []
        for task, target in plans:
            before = task.get("sprint")
            if set_sprint(task, target, stamp):
                touch(task, actor, stamp)
            moved.append({"id": task["id"], "from": before, "to": target})
        return moved

    return project.store.change(change)


# ==== lib/releases.py ====
# atm-lib: releases (docs/sprints-and-releases-design.md §3.3, SR-2.2; docs/layered-architecture.md §5.1): list them
# with their tasks by kind and stage, add one (with PRs, giving it their tasks, archived ones too), edit and archive one,
# and draft its notes. A task's release is its own, else its sprint's. Each write is one store change; the functions
# take keyword parameters (T-383).

STAGES = ("done", "in_pr", "merged")


def stage_of(vocab, task):
    """A done task's stage (DM-4.3): merged once its PR merged, in_pr while its PR is open, else done; None when it
    isn't done."""
    if kind_of(vocab, task.get("status")) != "done":
        return None
    pr = task.get("pr") if isinstance(task.get("pr"), dict) else None
    if pr and task.get("merged_at"):
        return "merged"
    return "in_pr" if pr else "done"


def release_overview(data, archived, include_archived):
    """The releases in order, each with its tasks (by effective release, both files) by kind and by stage."""
    vocab = vocabulary(data)
    counts = {r["id"]: ({k: 0 for k in KINDS}, {s: 0 for s in STAGES}) for r in vocab["releases"]}
    for task in list(data["tasks"]) + list(archived):
        if not isinstance(task, dict):
            continue
        release = effective_release(vocab, task)
        if release not in counts:
            continue
        by_kind, by_stage = counts[release]
        kind = kind_of(vocab, task.get("status"))
        if kind in by_kind:
            by_kind[kind] += 1
        stage = stage_of(vocab, task)
        if stage:
            by_stage[stage] += 1
    return [
        {
            **r,
            "tasks": sum(counts[r["id"]][0].values()),
            "by_kind": {k: n for k, n in counts[r["id"]][0].items() if n},
            "by_stage": {s: n for s, n in counts[r["id"]][1].items() if n},
        }
        for r in vocab["releases"]
        if include_archived or r["state"] != "archived"
    ]


def list_releases(project, include_archived=False):
    """The releases in order with their tasks by kind and stage. Returns {releases, has_releases}."""
    data = project.store.document()
    return {
        "releases": release_overview(data, project.store.list_archived(), include_archived),
        "has_releases": bool(vocabulary(data)["releases"]),
    }


def check_target(target):
    if target and not DATE.fullmatch(target):
        raise Invalid(f"--target {target!r}: use a date, YYYY-MM-DD")
    return target


def release_fields(release, name, target):
    """name and target on a release (None leaves one as it is); an empty value removes the field. Returns the names
    changed."""
    changed = []
    for key, value in (("name", name), ("target", check_target(target))):
        if value is None:
            continue
        if value == "":
            if key in release:
                del release[key]
                changed.append(key)
        elif release.get(key) != value:
            release[key] = value
            changed.append(key)
    return changed


def unknown_release(vocab, release_id):
    known = ", ".join(r["id"] for r in vocab["releases"]) or "none yet: atm release add <R>"
    return NotFound(f"no release {release_id!r}; the releases: {known}", 2)


def stored_release(data, release_id):
    """The release as stored in meta.releases (to change it), or an error naming the releases."""
    vocab = vocabulary(data)
    if find_release(vocab, release_id) is None:
        raise unknown_release(vocab, release_id)
    return next(r for r in data["meta"]["releases"] if r.get("id") == release_id)


def add_release(project, release_id, name=None, target=None, prs=(), released_at=None):
    """A planned release; with `prs` (pull request numbers) every task of those PRs, in both files, goes into it;
    with `released_at` it shipped already. Returns {release, tasks}: the tasks given to it."""
    if not RELEASE_ID.fullmatch(release_id):
        raise Invalid(f"{release_id!r}: a release ID is letters, digits, '.', '-' and '_' (at most 64)")
    prs = list(prs or [])
    if any(n <= 0 for n in prs):
        raise Invalid("--pr needs a pull request number")
    if released_at is not None:
        try:
            parse_iso(released_at)
        except ValueError:
            raise Invalid(f"--released-at needs a time like 2026-10-24T18:00:00Z, not {released_at!r}")

    def change(work):
        data, archive_data = work.document, work.archive()
        vocab = vocabulary(data)
        if find_release(vocab, release_id):
            raise Invalid(f"there is already a release {release_id!r}")
        release = {"id": release_id, "state": "planned"}
        release_fields(release, name, target)
        given = []
        # A release recovered from pull requests (SR-8.2): their tasks, in both files, go into it; the release records
        # the last PR given (the one that shipped it)
        for number in prs:
            own = []
            for task in list(data["tasks"]) + list(archive_data["tasks"]):
                pr = task.get("pr") if isinstance(task, dict) and isinstance(task.get("pr"), dict) else None
                if pr and pr.get("number") == number:
                    task["release"] = release_id
                    own.append(task["id"])
                    if isinstance(pr.get("url"), str):
                        release["pr"] = {"number": number, "url": pr["url"]}
            if not own:
                raise Invalid(f"no task is in pull request #{number}")
            given += own
        if released_at is not None:
            # One that shipped already: released at that time (its PR's merge, for one recovered from PRs)
            release["state"] = "released"
            release["released_at"] = released_at
        meta = data.setdefault("meta", {})
        meta["releases"] = list(meta.get("releases") or []) + [release]
        vocabulary(data)
        return {"release": release, "tasks": given}

    return project.store.change(change)


def edit_release(project, release_id, name=None, target=None):
    """Change a release's name or target (None leaves it, an empty value removes it). Returns {release, changed}."""

    def change(work):
        release = stored_release(work.document, release_id)
        changed = release_fields(release, name, target)
        if changed:
            vocabulary(work.document)
        return {"release": release, "changed": changed}

    return project.store.change(change)


def archive_release(project, release_id):
    """Archive a release: it leaves the open releases (`atm releases --all` shows it). Returns the release."""

    def change(work):
        release = stored_release(work.document, release_id)
        if release.get("state") == "archived":
            raise Invalid(f"the release {release['id']!r} is already archived")
        release["state"] = "archived"
        return release

    return project.store.change(change)


def outcome_of(task):
    """The task's last outcome note (what `atm done --note` said), or None."""
    notes = [n for n in task.get("notes") or [] if isinstance(n, dict) and n.get("kind") == "outcome"]
    return notes[-1].get("text") if notes else None


def release_notes(project, release_id):
    """A draft of the release's changes (§3.3): its done tasks (by effective release, both files) grouped by type in
    the project's order, each with its ID, name and outcome note; and the tasks in it not done yet. Returns
    {release, groups, open}."""
    data = project.store.document()
    vocab = vocabulary(data)
    release = find_release(vocab, release_id)
    if release is None:
        raise unknown_release(vocab, release_id)
    tasks = [t for t in list(data["tasks"]) + project.store.list_archived() if isinstance(t, dict)]
    own = [t for t in tasks if effective_release(vocab, t) == release_id]
    done = [t for t in own if kind_of(vocab, t.get("status")) == "done"]
    types = [(t["id"], t["label"]) for t in vocab["types"]]
    types += [(t, t) for t in dict.fromkeys(x.get("type") for x in done) if t not in dict(types)]
    groups = []
    for type_id, label in types:
        items = [{"id": t.get("id"), "name": t.get("name"), "outcome": outcome_of(t)} for t in done if t.get("type") == type_id]
        if items:
            groups.append({"type": type_id, "label": label, "tasks": items})
    open_tasks = [
        {"id": t.get("id"), "name": t.get("name"), "status": t.get("status")}
        for t in own
        if kind_of(vocab, t.get("status")) not in ("done", "skipped")
    ]
    return {"release": release, "groups": groups, "open": open_tasks}


# ==== lib/archive.py ====
# atm-lib: the archive (docs/v4/data-model.md §5; docs/layered-architecture.md §5.1): finished tasks moved out of the
# working tasks into the archive, so the working file stays small, and brought back.
#
# A move is one store change that tells the store it moves tasks into the archive (`work.moving_into_archive()`): the
# JSON-in-git store writes the archive first, then tasks.json; a restore the other way round. A task found in both
# keeps its working copy and loses the archive's: an archive that crashed halfway simply hasn't happened, an unarchive
# that crashed halfway is complete, and no task is ever lost.

def drop_duplicates(data, archive):
    """Tasks in both files keep their tasks.json copy (see the module docstring). Returns the dropped IDs."""
    ids = {t.get("id") for t in data["tasks"]}
    dropped = [t.get("id") for t in archive["tasks"] if t.get("id") in ids]
    archive["tasks"] = [t for t in archive["tasks"] if t.get("id") not in ids]
    return dropped


def kept_finished(data, moving, before, keep, sprints):
    """The finished tasks that stay in tasks.json, and why (T-352), as archivable() decides: in an open PR, finished
    in the last keep days, not before --before, not in the --sprint chosen. [{id, reason}], in file order."""
    vocab = vocabulary(data)
    days = archive_settings(data)["keep_days"] if keep is None else keep
    recent = now() - timedelta(days=days)
    moved = {t.get("id") for t in moving}
    kept = []
    for t in data["tasks"]:
        if not isinstance(t, dict) or t.get("id") in moved:
            continue
        if kind_of(vocab, t.get("status")) not in ("done", "skipped"):
            continue
        finished = finished_at(t)
        if isinstance(t.get("pr"), dict) and not t.get("merged_at"):
            reason = f"in open PR #{t['pr'].get('number')}"
        elif finished is not None and finished > recent:
            reason = f"finished in the last {days} days"
        elif before is not None and (finished is None or finished >= before):
            reason = f"finished on or after {before.date().isoformat()}"
        elif sprints is not None:
            reason = "not in the sprints chosen"
        else:
            continue
        kept.append({"id": t.get("id"), "reason": reason})
    return kept


def parse_before(before):
    """--before as a time: a date (2026-05-01) or a full ISO time; None without one."""
    if not before:
        return None
    try:
        return parse_iso(before if "T" in before else before + "T00:00:00Z")
    except ValueError:
        raise Invalid(f"--before {before}: use a date like 2026-05-01")


def plan_archive(data, archive, sprints, before, keep_days):
    """What an archive would do: (moving, dropped, kept, names). Changes `archive` (drops its duplicates)."""
    dropped = drop_duplicates(data, archive)
    moving = archivable(data, before, keep_days)
    names = None
    if sprints:
        vocab = vocabulary(data)
        names = set()
        for name in sprints:
            sprint = find_sprint(vocab, name)
            if sprint is None:
                raise NotFound(f"no sprint {name!r}", 2)
            names.add(sprint["id"])
        moving = [t for t in moving if (find_sprint(vocab, t.get("sprint")) or {}).get("id") in names]
    return moving, dropped, kept_finished(data, moving, before, keep_days, names)


def archive_finished(project, sprints=None, before=None, keep_days=None, dry_run=False):
    """`atm archive`: move the finished tasks (DM-5.1) to the archive, those of `sprints` only and finished before
    `before` if given, keeping those finished in the last `keep_days`. With `dry_run`, only what it would do. Returns
    {moved, dry_run, duplicates_dropped, kept, sizes}."""
    before = parse_before(before)
    if dry_run:
        # Nothing changes: worked out on copies of what the store reads
        data = {"meta": project.store.document().get("meta"), "tasks": list(project.store.list_tasks())}
        archive = {"meta": {}, "tasks": list(project.store.list_archived())}
        moving, dropped, kept = plan_archive(data, archive, sprints, before, keep_days)
        ids = [t.get("id") for t in moving]
        return {"moved": ids, "dry_run": True, "duplicates_dropped": dropped, "kept": kept, "sizes": project.store.sizes()}

    def change(work):
        data, archive = work.document, work.archive()
        moving, dropped, kept = plan_archive(data, archive, sprints, before, keep_days)
        ids = [t.get("id") for t in moving]
        if moving or dropped:
            work.moving_into_archive()
            # New IDs must never come back to an archived ID (DM-2.3)
            if not isinstance(data.get("meta"), dict):
                data["meta"] = {}
            data["meta"]["next_id"] = next_number(data)
            archive.setdefault("meta", {})["archived_at"] = now_iso()
            archive["tasks"].extend(moving)
            moved = set(ids)
            data["tasks"] = [t for t in data["tasks"] if t.get("id") not in moved]
            # The dashboard shows the count without loading the archive (dashboard D-5.4)
            data["meta"]["archived"] = len(archive["tasks"])
        return {"moved": ids, "dry_run": False, "duplicates_dropped": dropped, "kept": kept}

    result = project.store.change(change)
    return {**result, "sizes": project.store.sizes()}


def unarchive_tasks(project, ids):
    """`atm unarchive`: bring archived tasks back. Returns {restored, moves_back}: those the next archive would move
    straight back (finished and old, T-357)."""

    def change(work):
        data, archive = work.document, work.archive()
        drop_duplicates(data, archive)
        by_id = {t.get("id"): t for t in archive["tasks"]}
        missing = [i for i in ids if i not in by_id]
        if missing:
            # Already back, or never there at all: two different mistakes (T-357)
            live = {t.get("id") for t in data["tasks"]}
            here = [i for i in missing if i in live]
            unknown = [i for i in missing if i not in live]
            parts = ([f"{', '.join(here)} (already in tasks.json)"] if here else []) + (
                [f"{', '.join(unknown)} (no such task)"] if unknown else []
            )
            raise NotFound(f"not in the archive: {'; '.join(parts)}")
        data["tasks"].extend(by_id[i] for i in ids)
        # Still finished and old: the next atm archive would move it straight back (T-357)
        again = [t.get("id") for t in archivable(data, None, None) if t.get("id") in ids]
        if not isinstance(data.get("meta"), dict):
            data["meta"] = {}
        data["meta"]["archived"] = len(archive["tasks"]) - len(set(ids))
        archive["tasks"] = [t for t in archive["tasks"] if t.get("id") not in ids]
        return {"restored": list(ids), "moves_back": again}

    return project.store.change(change)


def archive_hint_now(project):
    """The suggestion to archive (CLI-5.1), or None: when the working tasks are over the threshold."""
    return archive_hint_for(project.store.document(), project.store.sizes()["tasks"])


def task_snapshot(project, filters):
    """The project's tasks as the REST API's v1 route gives them (docs/multi-project-api.md §2.5), in one snapshot:
    `meta` (as read: a tolerant read leaves out what doesn't work), `bytes` (how big the task data is), `tasks` (each
    with what atm derives for it, `task_views`), `problems` (what a tolerant read found wrong with the file) and, for
    the working tasks, `archive_hint` when atm suggests archiving (T-372)."""
    snapshot = {
        "meta": project.store.document().get("meta") or {},
        "bytes": project.store.sizes()["tasks"],
        "tasks": task_views(project, filters),
        "problems": read_problems(project),
    }
    hint = archive_hint_now(project) if not filters.archived else None
    if hint:
        snapshot["archive_hint"] = hint
    return snapshot


def project_stamp(project):
    """What changes when the project's data does ({tasks, archive, claims, head, copies}), for the REST API's ETags."""
    return project.store.stamp()


# ==== lib/code.py ====
# atm-lib: the project's workflow (meta.workflow: direct or pull-request, docs/v4/skills-and-workflows.md §1) and the
# code side of tasks: committing a task's files and recording the commit, the PR that contains tasks, its merge
# (docs/v4/atm-cli.md §4.5, data-model.md §4; docs/layered-architecture.md §2, §5.1).
#
# The commit goes through the project's version control (`project.vcs`); the files that travel with a code commit
# (tasks.json, for the JSON-in-git store) are the store's to say (`store.files_for_commit()`).

HTTP_URL = re.compile(r"https?://[^\s]+\Z", re.IGNORECASE)
CO_AUTHORED = re.compile(r"^co-authored-by:", re.IGNORECASE)


def get_workflow(project):
    """The project's workflow (meta.workflow with its defaults), or None when none is set."""
    return workflow_of({"meta": project.store.get_settings()})


def set_workflow(project, workflow_type, base=None, verify=None, max_rounds=None, commit_template=None, trailers=None, trailer=()):
    """Set the workflow: its type, and what is given of the rest; `trailers` forbid, require (with `trailer` lines) or
    none (removes the rule). Returns the workflow with its defaults."""
    if workflow_type not in WORKFLOW_TYPES:
        raise Invalid(f"atm project workflow set needs a type: {' or '.join(WORKFLOW_TYPES)}")
    changes = {"type": workflow_type}
    if base is not None:
        changes["base"] = base
    if verify is not None:
        changes["verify"] = verify
    if max_rounds is not None:
        changes["max_rounds"] = max_rounds
    if commit_template is not None:
        changes["commit_template"] = commit_template
    if trailers == "forbid":
        changes["trailers"] = "forbid"
    elif trailers == "require":
        if not trailer:
            raise Invalid("--trailers require needs at least one --trailer LINE")
        changes["trailers"] = {"require": list(trailer)}

    def change(work):
        data = work.document
        if not isinstance(data.get("meta"), dict):
            data["meta"] = {}
        current = data["meta"].get("workflow") if isinstance(data["meta"].get("workflow"), dict) else {}
        workflow = {**current, **changes}
        if trailers == "none":
            workflow.pop("trailers", None)
        data["meta"]["workflow"] = workflow
        return workflow_of(data)  # validates before anything is written

    return project.store.change(change)


def commit_message(text, trailers):
    """Apply the workflow's trailer rule: forbid removes Co-Authored-By lines, require adds the missing lines."""
    if trailers == "forbid":
        text = "\n".join(line for line in text.splitlines() if not CO_AUTHORED.match(line.strip())).rstrip()
    elif isinstance(trailers, dict):
        missing = [line for line in trailers["require"] if line not in text.splitlines()]
        if missing:
            text = text.rstrip() + "\n\n" + "\n".join(missing)
    return text


def record_on_task(project, task_id, sha):
    """The task's commit is `sha` (the SHA is known only once the commit exists: a change for the next commit)."""

    def change(work):
        task = find_task(work.document, task_id, archived_in(work))
        task["commit"] = sha
        touch(task, project.actor, now_iso())
        return task

    return project.store.change(change)


def repository_paths(project, files, cwd):
    """The files named relative to `cwd`, relative to the repository instead (what commit_task takes), checked as a
    commit checks them: no directory, no pattern, nothing outside the repository."""
    vcs = project.vcs
    if vcs is None or not vcs.in_repository():
        raise AtmFailure("atm vcs commit needs a git repository")
    return vcs.paths(files, cwd)


def folder_changes(project, task_id, folders, cwd):
    """`atm commit --dir` (T-405): every changed file under the folders (named relative to `cwd`), relative to the
    repository, for commit_task. Refused as a whole when one of them is a file another agent's claimed task lists in
    its `files` (the file, or a folder holding it): the case committing only named files guards against, another
    agent's unfinished work in the same tree."""
    vcs = project.vcs
    if vcs is None or not vcs.in_repository():
        raise AtmFailure("atm vcs commit needs a git repository")
    changed = vcs.changes_under(list(folders), cwd)
    claims = project.store.read_claims()["claims"]
    for claim in claims.values():
        if not isinstance(claim, dict) or claim.get("task") == task_id or claim.get("agent") == project.actor:
            continue
        other = project.store.get_task(claim.get("task"))
        listed = [f for f in (other or {}).get("files") or [] if isinstance(f, str)]
        for path in changed:
            owner = next((f for f in listed if path == f.rstrip("/") or path.startswith(f.rstrip("/") + "/")), None)
            if owner:
                raise Refused(
                    f"--dir would commit {path}, which {claim['task']} lists ({owner}), claimed by {claim.get('agent')}: "
                    "name your own files with --file"
                )
    return changed


def commit_task(project, task_id, files, message=None):
    """`atm commit`: commit exactly `files` (relative to the repository; repository_paths turns names relative to a
    folder into these) plus the task data that travels with code commits, with the workflow's message, then record
    the commit on the task. Returns {task, commit, message, files}."""
    vcs = project.vcs
    if vcs is None or not vcs.in_repository():
        raise AtmFailure("atm vcs commit needs a git repository")
    if not files:
        raise Invalid("atm vcs commit needs --file (once per file the task changed), --dir FOLDER, or --record REV")
    store = project.store
    # The one task, or the error find_task gives (saying when it is archived)
    task = store.get_task(task_id) or find_task({"tasks": []}, task_id, store.get_archived_task)
    workflow = workflow_of({"meta": store.get_settings()}) or WORKFLOW_DEFAULTS
    text = message if message is not None else workflow["commit_template"]
    for key in ("id", "name", "phase", "type"):
        text = text.replace("{" + key + "}", str(task.get(key, "")))
    text = commit_message(text, workflow.get("trailers"))
    if not text.strip():
        raise Invalid("the commit message is empty")
    picked = vcs.paths(files)
    picked += [f for f in project.store.files_for_commit() if f not in picked]
    sha = vcs.commit(picked, text)
    task = record_on_task(project, task_id, sha)
    return {"task": task["id"], "commit": sha, "message": text, "files": picked}


def record_commit(project, task_id, rev):
    """`atm commit --record` (T-343): record an existing commit on the current branch, e.g. the one a rebase or a
    cherry-pick made of the task's commit, without committing anything. Returns {task, commit, recorded, summary,
    hint}: `hint` when the commit's message doesn't mention the task."""
    vcs = project.vcs
    if vcs is None or not vcs.in_repository():
        raise AtmFailure("atm vcs commit needs a git repository")
    if rev.startswith("-"):
        raise Invalid(f"--record takes a revision, not an option: {rev!r}")
    sha = vcs.resolve(rev)
    if not sha:
        raise NotFound(f"no commit {rev} in this repository")
    if not vcs.in_history(sha):
        raise AtmFailure(f"{sha[:7]} is not on the current branch (an ancestor of HEAD)")
    text = vcs.message(sha)
    task = record_on_task(project, task_id, sha)
    hint = None
    if not re.search(rf"(?<![\w-]){re.escape(task['id'])}(?![\w-])", text):
        hint = f"its message doesn't mention {task['id']}: is it the task's commit?"
    return {"task": task["id"], "commit": sha, "recorded": True, "summary": text.splitlines()[0] if text else "", "hint": hint}


def record_pr(project, ids, number, url):
    """`atm pr`: the PR that contains the tasks (DM-4.3: the tasks are then In PR). Returns the task IDs."""
    if number <= 0:
        raise Invalid("--number must be a positive whole number")
    # The dashboard links to it: only a web address, never javascript: or data: (docs/security-review.md F9)
    if not HTTP_URL.match(url):
        raise Invalid(f"--url must be an http:// or https:// address, not {url!r}")

    def change(work):
        archived = archived_in(work)
        found = [find_task(work.document, task_id, archived) for task_id in ids]
        stamp = now_iso()
        for task in found:
            task["pr"] = {"number": number, "url": url}
            task.pop("merged_at", None)
            touch(task, project.actor, stamp)
        return [t["id"] for t in found]

    return project.store.change(change)


def release_with_merge(data, merged, pr_number, at):
    """Release the planned releases the merged PR completes (SR-3.7): every task of the release in tasks.json (its own
    release, or its sprint's) is in this PR and merged; archived and skipped tasks don't hold it back. Sets its state,
    `pr` and `released_at`. Returns (the released IDs, {release: the tasks still keeping it planned})."""
    vocab = vocabulary(data)
    touched = {effective_release(vocab, t) for t in merged} - {None}
    released, pending = [], {}
    for release in (data.get("meta") or {}).get("releases") or []:
        if not isinstance(release, dict) or release.get("id") not in touched:
            continue
        if release.get("state", "planned") != "planned":
            continue
        own = [
            t
            for t in data["tasks"]
            if isinstance(t, dict)
            and effective_release(vocab, t) == release["id"]
            and kind_of(vocab, t.get("status")) != "skipped"
        ]
        waiting = [
            t.get("id")
            for t in own
            if not (isinstance(t.get("pr"), dict) and t["pr"].get("number") == pr_number and t.get("merged_at"))
        ]
        if waiting:
            pending[release["id"]] = waiting
            continue
        url = next((t["pr"].get("url") for t in merged if isinstance(t["pr"].get("url"), str)), None)
        release["state"] = "released"
        release["pr"] = {"number": pr_number, **({"url": url} if url else {})}
        release["released_at"] = at
        released.append(release["id"])
    return released, pending


def record_merge(project, pr, at=None):
    """`atm merged`: PR `pr` was merged (at `at`, default now), on every task that records it (DM-4.3: the tasks are
    then Merged); the releases it completes are released. Returns {pr, tasks, already, released, pending}."""
    if at is not None:
        try:
            parse_iso(at)
        except ValueError:
            raise Invalid(f"--at needs a time like 2026-10-24T18:00:00Z, not {at!r}")

    def change(work):
        data = work.document
        tasks = [
            t for t in data["tasks"] if isinstance(t, dict) and isinstance(t.get("pr"), dict) and t["pr"].get("number") == pr
        ]
        if not tasks:
            raise NotFound(f"no task records PR #{pr} (atm vcs pr records it)")
        stamp = now_iso()
        changed = [t for t in tasks if not t.get("merged_at")]
        for task in changed:
            task["merged_at"] = at or stamp
            touch(task, project.actor, stamp)
        released, pending = release_with_merge(data, tasks, pr, at or stamp)
        return {
            "pr": pr,
            "tasks": [t["id"] for t in changed],
            "already": [t["id"] for t in tasks if t not in changed],
            "released": released,
            "pending": pending,
        }

    return project.store.change(change)


# ==== lib/projects.py ====
# atm-lib: the user's projects, the ones the dashboard shows (`atm projects`; docs/layered-architecture.md §3.2, §5.1):
# list, add, remove and prune them in the registry (`registry`, a db Registry: boot opens it).

def list_projects(registry):
    """{projects: the entries, each with `missing`, damaged: where an unreadable registry was moved, or None}."""
    return registry.list_projects()


def add_project(registry, target):
    """Register the project in the folder `target` (the current one when None). {result, project, damaged}."""
    return registry.add_project(target)


def remove_project(registry, target):
    """Unregister the project with this ID or path. {removed, damaged}."""
    return registry.remove_project(target)


def prune_projects(registry):
    """Unregister the projects whose task folder is gone. {removed, damaged}."""
    return registry.prune()


# ==== lib/settings.py ====
# atm-lib: the project's settings in meta (docs/layered-architecture.md §3.1, §5.1): the keys `init --meta-file`
# sets, the project's id, the repository's web address and the logo. Each change is one change of the store (§3.3).

def _meta(data):
    if not isinstance(data.get("meta"), dict):
        data["meta"] = {}
    return data["meta"]


def init_meta(project, new):
    """`init --meta-file`: merge `new`'s keys into meta (project, description, vocabularies, ...), creating the task
    data when there is none, so /atm:create-tasks never writes the file itself. The vocabularies are checked before
    anything is written; meta.id is kept. Returns (created, the keys set)."""
    new = {k: v for k, v in new.items() if k != "id"}
    store = project.store
    if not store.exists():
        data = {"meta": {"created": now().strftime("%Y-%m-%d"), **new}, "tasks": []}
        vocabulary(data)  # raises, naming the problem, before anything is written
        store.create(data)
        return True, sorted(new)

    def change(work):
        _meta(work.document).update(new)
        vocabulary(work.document)

    store.change(change)
    return False, sorted(new)


def ensure_project_id(project):
    """Give the project a meta.id if it has none: the dashboard keeps each project's saved view under it (projects
    served from the same port share browser storage). Returns the new id, or None if there was one already."""
    if not project.store.exists():
        return None

    def change(work):
        meta = _meta(work.document)
        if isinstance(meta.get("id"), str) and meta["id"]:
            return None
        meta["id"] = os.urandom(4).hex()
        return meta["id"]

    return project.store.change(change)


def ensure_repository(project):
    """Fill meta.repository from the version control's remote when there is none. Returns the address set, or None."""
    if not project.store.exists() or project.vcs is None:
        return None
    url = repository_url(project.vcs.remote_url() or "")
    if not url:
        return None

    def change(work):
        meta = _meta(work.document)
        if meta.get("repository"):
            return None
        meta["repository"] = url
        return url

    return project.store.change(change)


def get_repository(project):
    """meta.repository, or None."""
    return project.store.get_settings().get("repository") or None


def set_repository(project, url, previous=None):
    """Set meta.repository to `url` (an http(s) URL or a git remote). When the repository moved (from `previous`, else
    the address set before), the tasks' PR links move with it, in the working tasks and the archive (T-273). Returns
    {repository, moved_links, previous}."""
    address = repository_url(url)
    if not address:
        raise Invalid(f"{url!r} is not a repository address: give an http(s) URL or a git remote")
    old = None
    if previous is not None:
        old = repository_url(previous)
        if not old:
            raise Invalid(f"--from {previous!r} is not a repository address: give an http(s) URL or a git remote")

    def change(work):
        meta = _meta(work.document)
        before = old or repository_url(meta.get("repository") or "")
        meta["repository"] = address
        moved = 0
        if before and before != address:
            moved = move_pr_links(work.document.get("tasks") or [], before, address)
            moved += move_pr_links(work.archive()["tasks"], before, address)
        return {"repository": address, "moved_links": moved, "previous": before}

    return project.store.change(change)


def get_logo(project):
    """The project's logo: {logo: its path in the project, source: "set" or "found", file, type} or None (store's
    `logo`), and `set`, what meta.logo says."""
    settings = project.store.get_settings()
    return {"found": project.store.logo(settings), "set": settings.get("logo")}


def set_logo(project, path):
    """Set meta.logo to the image at `path` (absolute, or relative to where atm runs): kept relative to the project's
    working copy; the store refuses one it can't serve. Returns the path kept."""
    root = project.store.working_copy()
    relative = os.path.relpath(os.path.abspath(path), os.path.realpath(root)).replace(os.sep, "/") if root else path
    project.store.check_logo(relative)

    def change(work):
        _meta(work.document)["logo"] = relative

    project.store.change(change)
    return relative


def unset_logo(project):
    """Remove meta.logo. Returns what it was, or None when none was set."""
    return project.store.change(lambda work: _meta(work.document).pop("logo", None))


# ==== lib/worktrees.py ====
# atm-lib: a project's working copies (git worktrees, for the JSON-in-git store; docs/layered-architecture.md §3.2):
# the dashboard shows them on one board, each task from the working copy where it got furthest.

def working_copies(project):
    """The project's working copies, main first ([{path, branch, main}]); [] for a store without them."""
    return project.store.working_copies()


def copy_views(project, folder):
    """The tasks of the working copy at `folder` as views ({task, derived}), each worked out on that copy's own data
    (§11.10 A): what `atm next` would say there, so a dependency done in another worktree counts once the branches
    merge. By task ID; {} when the copy can't be read."""
    try:
        views = task_views(Project(project.store.copy_store(folder)), Filters())
    except AtmFailure:
        return {}
    return {v["task"].get("id"): v for v in views if isinstance(v["task"].get("id"), str)}


def worktree_copies(project):
    """The project's working copies, main first ([{path, branch, main}]), and for each other one the tasks whose copy
    there differs from the main one's (or that only it has), under `tasks`, as views ({task, derived}) worked out in
    that working copy. A copy of a task decided in another working copy (answered, reviewed: the claims store's
    `resolved`, T-204) is left out: it is stale until atm next runs there, and would otherwise outrank the decision.
    [] for a store without working copies."""
    store = project.store
    trees = store.working_copies()
    resolved = store.read_claims()["resolved"]
    stale = {(r.get("worktree"), task_id) for task_id, r in resolved.items() if isinstance(r, dict)}
    listed, main = [], {}
    for tree in trees:
        tasks = store.copy_tasks(tree["path"])
        if tree["main"]:
            main = {t["id"]: t for t in tasks}
            listed.append(tree)
            continue
        differing = [t for t in tasks if main.get(t["id"]) != t and (tree["path"], t["id"]) not in stale]
        views = copy_views(project, tree["path"]) if differing else {}
        listed.append(
            {**tree, "tasks": [views.get(t["id"]) or {"task": t, "derived": {}} for t in differing]}
        )
    return listed


# ==== lib/documents.py ====
# A project's files for the document viewer (docs/compmd-viewer-design.md §4): which ones atm hands out, what each
# is, and the CompMD document the dashboard renders for it. atm renders nothing itself: a code file becomes a fenced
# block of its language, an image a Markdown image, and CompMD does the rest.
#
# The rules are here; the reading is the version control's (the project's working copy: vcs `file_stamp`, `read_file`,
# `ignored`, `tracked`, `head`).

DOC_MAX_PATH = 512
DOC_MAX_BYTES = 1_000_000  # a document, code or text
DOC_MAX_IMAGE_BYTES = 5_000_000
# How much of a file says whether it is text
DOC_SNIFF_BYTES = 8192

DOC_MARKDOWN = (".md", ".markdown")  # .comp.md ends in .md
DOC_IMAGES = {
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".gif": "image/gif",
    ".webp": "image/webp",
    ".svg": "image/svg+xml",
}
# A code file's fence language, by extension (or by whole name, for files without one)
DOC_LANGUAGES = {
    ".py": "python",
    ".pyi": "python",
    ".js": "javascript",
    ".mjs": "javascript",
    ".cjs": "javascript",
    ".jsx": "jsx",
    ".ts": "ts",
    ".mts": "ts",
    ".cts": "ts",
    ".tsx": "tsx",
    ".svelte": "svelte",
    ".vue": "vue",
    ".json": "json",
    ".jsonc": "jsonc",
    ".yaml": "yaml",
    ".yml": "yaml",
    ".toml": "toml",
    ".ini": "ini",
    ".cfg": "ini",
    ".xml": "xml",
    ".html": "html",
    ".htm": "html",
    ".css": "css",
    ".scss": "scss",
    ".sass": "sass",
    ".less": "less",
    ".sh": "sh",
    ".bash": "bash",
    ".zsh": "zsh",
    ".fish": "fish",
    ".ps1": "powershell",
    ".bat": "bat",
    ".rs": "rust",
    ".go": "go",
    ".java": "java",
    ".kt": "kotlin",
    ".kts": "kotlin",
    ".scala": "scala",
    ".groovy": "groovy",
    ".gradle": "groovy",
    ".c": "c",
    ".h": "c",
    ".cc": "cpp",
    ".cpp": "cpp",
    ".cxx": "cpp",
    ".hpp": "cpp",
    ".cs": "csharp",
    ".fs": "fsharp",
    ".swift": "swift",
    ".m": "objectivec",
    ".rb": "ruby",
    ".php": "php",
    ".pl": "perl",
    ".lua": "lua",
    ".r": "r",
    ".jl": "julia",
    ".dart": "dart",
    ".ex": "elixir",
    ".exs": "elixir",
    ".erl": "erlang",
    ".hs": "haskell",
    ".clj": "clojure",
    ".ml": "ocaml",
    ".zig": "zig",
    ".nim": "nim",
    ".sql": "sql",
    ".graphql": "graphql",
    ".proto": "protobuf",
    ".tf": "hcl",
    ".hcl": "hcl",
    ".mermaid": "mermaid",
    ".mmd": "mermaid",
    ".diff": "diff",
    ".patch": "diff",
    "Dockerfile": "dockerfile",
    "Makefile": "makefile",
}
# Files that hold keys, refused whatever git says (§4b)
DOC_KEY_FILES = re.compile(r"(\.(pem|key|p12|pfx|keystore|jks)|^id_(rsa|dsa|ecdsa|ed25519)(\..*)?)$", re.I)
# The start of each image type, so a file named .png is one
DOC_IMAGE_MAGIC = {
    "image/png": (b"\x89PNG\r\n\x1a\n",),
    "image/jpeg": (b"\xff\xd8\xff",),
    "image/gif": (b"GIF87a", b"GIF89a"),
}


def document_path(path):
    """The path a request names, checked (§4b): relative to the project's root, `/`-separated, normalized (no `..`,
    `.` or empty segment), no hidden segment (`.git`, `.env`, `.agentic-task-manager`), no NUL or backslash, at most
    DOC_MAX_PATH characters. Invalid otherwise; Refused for a hidden or key file."""
    if not isinstance(path, str) or not path or len(path) > DOC_MAX_PATH:
        raise Invalid("a file path, relative to the project, of at most 512 characters")
    if "\0" in path or "\\" in path or path.startswith("/"):
        raise Invalid(f"{path}: a relative path with / between its parts")
    segments = path.split("/")
    if any(s in ("", ".", "..") for s in segments):
        raise Invalid(f"{path}: a path inside the project, without . or .. parts")
    if any(s.startswith(".") for s in segments):
        raise Refused(f"{path}: hidden files and folders aren't shown")
    if DOC_KEY_FILES.search(segments[-1]):
        raise Refused(f"{path}: key files aren't shown")
    return path


def document_kind(path):
    """What a file is by its name: (kind, language or content type). `markdown`, `image` (with its content type),
    `code` (with its fence language), or `text`; `binary` is found only by reading it."""
    name = path.rsplit("/", 1)[-1]
    extension = os.path.splitext(name)[1].lower()
    if extension in DOC_MARKDOWN:
        return "markdown", None
    if extension in DOC_IMAGES:
        return "image", DOC_IMAGES[extension]
    language = DOC_LANGUAGES.get(extension) or DOC_LANGUAGES.get(name)
    return ("code", language) if language else ("text", None)


def _checked(project, path):
    """(path, kind, language or content type, stamp) of a file the viewer may show: its path checked, a regular file
    reached through no symlink, not ignored by version control, not too large. NotFound, Refused or TooLarge."""
    path = document_path(path)
    vcs = project.vcs
    if vcs is None:
        raise NotFound("this project has no working copy to read files from")
    stamp = vcs.file_stamp(path)
    if stamp is None:
        raise NotFound(f"{path}: not in the project")
    if vcs.ignored(path):
        raise Refused(f"{path}: git ignores it, so it isn't shown")
    kind, detail = document_kind(path)
    limit = DOC_MAX_IMAGE_BYTES if kind == "image" else DOC_MAX_BYTES
    if stamp[0] > limit:
        raise TooLarge(f"{path}: larger than {limit // 1_000_000} MB")
    return path, kind, detail, stamp


def document_stamp(project, path):
    """(size, nanosecond mtime) of a file the viewer may show, after the same checks as read_document: for the
    REST API's ETag, so an unchanged file is answered without reading it."""
    return _checked(project, path)[3]


def _fence(text, language):
    """`text` as a fenced code block of `language`, the fence longer than any run of backticks in it."""
    longest = max((len(run) for run in re.findall(r"`+", text)), default=0)
    fence = "`" * max(3, longest + 1)
    body = text if text.endswith("\n") or not text else text + "\n"
    return f"{fence}{language or ''}\n{body}{fence}\n"


def _is_binary(data):
    """A NUL in its first bytes, or bytes that aren't UTF-8."""
    if b"\0" in data[:DOC_SNIFF_BYTES]:
        return True
    try:
        data.decode("utf-8")
    except UnicodeDecodeError:
        return True
    return False


def _image_matches(data, content_type):
    if content_type == "image/svg+xml":
        return b"<svg" in data[:DOC_SNIFF_BYTES].lower()
    if content_type == "image/webp":
        return data[:4] == b"RIFF" and data[8:12] == b"WEBP"
    return data.startswith(DOC_IMAGE_MAGIC[content_type])


def _link_target(name):
    """A file name as a Markdown link destination, in angle brackets when it has spaces or parentheses."""
    return f"<{name}>" if re.search(r"[\s()<>]", name) else name


def read_document(project, path):
    """A file of the project for the viewer (§4a): {path, kind, language, text, document, line_offset, lines, bytes,
    modified_at, tracked, commit}. `document` is the CompMD document to render: a Markdown file as it is, code and
    text in a fence (`line_offset` 1: the fence's first line comes before the file's), an image as a Markdown image
    of its own name (relative, which the dashboard points at the files route), a binary file as a line saying so.
    `text` is the file as it is (None for an image or a binary file); `lines` counts its lines. `tracked` and
    `commit` (the current commit) are None outside version control; `submodule` is {path, commit, repository} for a
    file in a git submodule, else None. NotFound, Refused or TooLarge."""
    path, kind, detail, stamp = _checked(project, path)
    data = project.vcs.read_file(path)
    name = path.rsplit("/", 1)[-1]
    text = None
    line_offset = 0
    if kind == "image":
        if not _image_matches(data, detail):
            kind = "binary"
            document = f"*{name} isn't the image its name says: {len(data)} bytes.*\n"
        else:
            document = f"![{name}]({_link_target(name)})\n"
    elif _is_binary(data):
        kind, detail = "binary", None
        document = f"*{name} is a binary file: {len(data)} bytes.*\n"
    else:
        text = data.decode("utf-8")
        if kind == "markdown":
            document = text
        else:
            document = _fence(text, detail)
            line_offset = 1
    lines = 0 if text is None else len(text.splitlines())
    modified = datetime.fromtimestamp(stamp[1] / 1e9, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    return {
        "path": path,
        "kind": kind,
        "language": detail if kind == "code" else None,
        "text": text,
        "document": document,
        "line_offset": line_offset,
        "lines": lines,
        "bytes": len(data),
        "modified_at": modified,
        "tracked": project.vcs.tracked(path),
        "commit": project.vcs.head(),
        # A file in a submodule lives in the submodule's repository: its folder, commit and web address (T-408)
        "submodule": project.vcs.submodule_of(path),
    }


def read_image(project, path):
    """(bytes, content type) of an image file of the project, for an <img> (the files route): only the image types
    the viewer shows, whose bytes match their name. Refused for any other file, as well as read_document's errors."""
    path, kind, content_type, _ = _checked(project, path)
    if kind != "image":
        raise Refused(f"{path}: only images are served as files")
    data = project.vcs.read_file(path)
    if not _image_matches(data, content_type):
        raise Refused(f"{path}: isn't the image its name says")
    return data, content_type


# ==== boot/identity.py ====
# Who runs a command (the actor, DM-4.5), from the environment: --agent, ATM_AGENT, the Claude Code session (per
# linked git worktree), git's user name, or user@host. Boot's, because it asks the store (which worktree a folder is)
# and the version control (the user's name).

def session_identity(session_id, cwd):
    """A Claude Code session's identity: "claude:" and 8 characters of its ID; in a linked git worktree, also
    "@<worktree folder>". Subagents share their parent's session ID, so agents spawned in separate worktrees still get
    identities of their own. The main worktree keeps the plain form, so its claims and history still match."""
    identity = "claude:" + str(session_id)[:8]
    root = project_root(cwd)
    if main_worktree(root) != root:
        identity += "@" + os.path.basename(root)
    return identity


def agent_identity(agent=None):
    """Who runs the command (the actor, DM-4.5): --agent, else ATM_AGENT, else this Claude Code session (per linked
    worktree, see session_identity), else the git user's name (a person in a terminal), else user@host."""
    if agent:
        return agent
    if os.environ.get("ATM_AGENT"):
        return os.environ["ATM_AGENT"]
    if os.environ.get("CLAUDE_CODE_SESSION_ID"):
        return session_identity(os.environ["CLAUDE_CODE_SESSION_ID"], os.getcwd())
    name = GitVcs(project_root(os.getcwd())).user_name()
    if name:
        return name
    user = os.environ.get("USER") or os.environ.get("USERNAME") or "user"
    return f"{user}@{socket.gethostname()}"


# ==== boot/open.py ====
# Opening a project, and the user's registry of projects: what every store has (docs/layered-architecture.md §1,
# §11.2). Boot is the one place that knows which store and which version control atm runs on: today the JSON-in-git
# store and git. What only the JSON-in-git store has (its paths, migrations, merge driver, registering by folder) is
# `boot/jsongit.py`'s.

def open_project(cwd, actor=None, tolerant=False):
    """The project at or above `cwd`. `tolerant`: read a malformed task file as far as it can be read (T-377)."""
    return _project_at(find_paths(cwd), actor, tolerant)


def project_opener(folder):
    """A way to open the project at `folder` again and again, cheaply: `opener(actor=None, tolerant=False)` gives the
    project afresh each time (atm serve, one snapshot per request), finding where its data is only once (that runs
    git). Its folder's symlink checks run on every open (docs/security-review.md F7, F8)."""
    found = {}

    def opener(actor=None, tolerant=False):
        if "paths" not in found:
            found["paths"] = find_paths(folder)
        return _project_at(found["paths"], actor, tolerant)

    return opener


def _project_at(paths, actor=None, tolerant=False):
    """The project whose task files `paths` locates. The task folder's symlink checks run each time, so a file linked
    after the paths were found is refused too."""
    check_task_folder(paths["root"], paths["dir"])
    return Project(JsonGitStore(paths, tolerant=tolerant), GitVcs(paths["root"]), actor)


def open_memory_project(tasks=(), archive=(), meta=None, actor=None):
    """A project on the in-memory store, with no version control: for other tools' tests (and atm-lib's own), so they
    never import `atm.db`. The lists given are copied; the project's changes stay in memory."""
    copy = lambda items: [dict(item) for item in items]
    return Project(MemoryStore(tasks=copy(tasks), archive=copy(archive), meta=dict(meta or {})), None, actor)


def open_registry(home=None):
    """The user's registry of projects (the dashboard's list)."""
    return FileRegistry(home)


# ==== boot/jsongit.py ====
# What only the JSON-in-git store has, for the commands about it (docs/layered-architecture.md §3.2, §11.2): where a
# project's task files are (`atm paths`), its migrations (`atm migrate`, and `atm serve` before showing a project), its
# merge driver (`atm merge-driver`, `atm init`), its lock (the lock test), and registering a project on the dashboard by
# its folder. A store that isn't files in git has none of these: the commands that use them (`cli/store.py`, `atm
# init`'s setup) are this store's. What every store has is `boot/open.py`'s.

def task_paths(cwd):
    """`atm paths`: where the project's task files are (the JSON-in-git store's own locations)."""
    return find_paths(cwd)


def register_project(project, wait=True):
    """List the project on the dashboard, by its folder (every command that sets a project up does). Returns what the
    registry did."""
    return register(project.store.paths, wait)


def dashboard_id(project):
    """(id, listed) of the project on the dashboard, after registering it: the id the registry gave it (meta.id, or
    another when that one is taken) and whether it is listed (meta.dashboard.listed)."""
    register(project.store.paths)
    return registered_id(project.store.paths)


def set_up_store(project):
    """`atm init`: set the store up in its working copy: in git, the merge driver for the task files (registered in
    the clone, `.gitattributes` staged). Returns {git, gitattributes_added}."""
    paths = project.store.paths
    in_repository = project.vcs.in_repository()
    return {"git": in_repository, "gitattributes_added": ensure_merge_driver(paths) if in_repository else False}


def migrate_folder(cwd, register_it=True):
    """`atm migrate` (and `atm serve`, before it shows a project): bring the task folder at or above `cwd` up to date
    (docs/v4/atm-cli.md). Returns {result, lines: what it did, in words, tasks_exists}; with `register_it`, the
    project is listed on the dashboard too (every skill runs migrate first)."""
    paths = find_paths(cwd)
    result = migrate(paths)
    exists = os.path.isfile(paths["tasks"])
    if register_it and exists:
        register(paths)
    return {"result": result, "lines": describe_migration(result, paths), "tasks_exists": exists}


def merge_versions(base, ours, theirs):
    """`atm merge-driver` (git runs it): merge the three versions of a task file task by task, writing the result
    over `ours`. Returns the conflicts (this branch's version was kept for each)."""
    return merge_files(base, ours, theirs)


def hook_project(cwd, actor):
    """The project the plugin's hook (`heartbeat --auto`) works on in `cwd`, after listing it on the dashboard (one
    read of the registry when it is listed already); None when there are no task files there."""
    paths = find_paths(cwd)
    if not paths["tasks_exists"]:
        return None
    try:
        register_if_needed(paths)
    except Exception:  # noqa: BLE001 - the hook must never fail on the registry
        pass
    return _project_at(paths, actor)  # the paths found once: the hook runs after every tool call


def hold_lock_for_test(cwd, seconds):
    """`atm _lock-test`: hold the task folder's lock for `seconds`, logging to .lock-test.log, so tests can check that
    two processes never hold it at once."""
    task_dir = find_paths(cwd)["dir"]
    log = os.path.join(task_dir, ".lock-test.log")
    with locked(task_dir):
        with open(log, "a", encoding="utf-8") as f:
            f.write(f"acquire {os.getpid()}\n")
        time.sleep(seconds)
        with open(log, "a", encoding="utf-8") as f:
            f.write(f"release {os.getpid()}\n")


# ==== cli/render.py ====
# Text for people and agents: how the CLI shows atm-lib's results. Short and stable, for agents to read (CLI-3.3).

def report_expired(expired):
    """On stderr, the claims a command found expired (their tasks went back to todo)."""
    for claim in expired:
        sys.stderr.write(f"atm: lease of {claim['task']} by {claim.get('agent')} expired; it is available again\n")


def view_row(view):
    """The fields `atm list` shows of a task, from atm-lib's view of it (task_views), as a dict for --json."""
    task, derived = view["task"], view["derived"]
    fields = {
        "id": task.get("id"),
        "status": task.get("status"),
        "kind": derived.get("kind"),
        "sprint": task.get("sprint"),
        "priority": derived.get("priority"),
        "labels": task.get("labels", []),
        "name": task.get("name", ""),
    }
    if isinstance(task.get("pr"), dict):
        fields["pr"] = task["pr"]
        fields["merged_at"] = task.get("merged_at")
    return fields


def list_lines(rows):
    """One line per task, as `atm list` prints them: ID, status, priority, labels, name (rows as view_row gives
    them, or atm-lib's `summary`)."""
    id_width = max(len(str(t.get("id"))) for t in rows)
    status_width = max(len(str(t.get("status"))) for t in rows)
    for t in rows:
        labels = t.get("labels") or []
        yield (
            f"{str(t.get('id')).ljust(id_width)}  {str(t.get('status')).ljust(status_width)}  "
            f"{t.get('priority')}  "
            + (f"[{', '.join(labels)}] " if labels else "")
            + str(t.get("name", ""))
        )


def render_task(view):
    """A task in full, as text (atm-lib's view of it: `show_tasks`, or a status change's `view`): what an agent needs
    to work on it. `atm start` prints the same text as `atm show`."""
    task = view["task"]
    labels = task.get("labels") or []
    lines = [
        f"{task.get('id')}  {task.get('name', '')}",
        f"  {task.get('status')} · {task.get('sprint') or 'backlog'}"
        # A task from the archive says so (found in T-277, T-353)
        + (" · archived" if view["archived"] else "")
        + f" · {task.get('type')} · {view['priority']}"
        + (f" · labels: {', '.join(labels)}" if labels else ""),
    ]
    if view["dependencies"]:
        lines.append(
            "  depends on: "
            + ", ".join(
                f"{d['id']} (missing)"
                if d["status"] is None
                else f"{d['id']} ({d['status']}" + (", archived" if d["archived"] else "") + ")"
                for d in view["dependencies"]
            )
        )
    # How it ended, when it isn't the default (done, or won't do for a skipped task), and its relations (SR-2.8)
    if task.get("resolution"):
        lines.append(
            f"  resolution: {task['resolution']}" + (f" (of {task['duplicate_of']})" if task.get("duplicate_of") else "")
        )
    if view["related"]:
        lines.append("  related: " + ", ".join(view["related"]))
    if view["duplicated_by"]:
        lines.append("  duplicated by: " + ", ".join(view["duplicated_by"]))
    for key in ("requirements", "use_cases", "files"):
        if task.get(key):
            lines.append(f"  {key.replace('_', ' ')}: {', '.join(task[key])}")
    who = []
    for what, at, by in (
        ("created", "created_at", task.get("created_by")),
        ("started", "started_at", view["started_by"]),
        ("review requested", "review_requested_at", None),
        ("completed", "completed_at", task.get("completed_by")),
    ):
        if task.get(at):
            who.append(f"{what} {task[at]}" + (f" by {by}" if by else ""))
    if who:
        lines.append("  " + " · ".join(who))
    if task.get("commit"):
        lines.append(f"  commit: {task['commit']}")
    description = (task.get("description") or "").strip()
    if description:
        lines.append("  description:")
        lines.extend("    " + line for line in description.splitlines())
    notes = task.get("notes") or []
    if notes:
        lines.append("  notes:")
        for n in notes:
            lines.append(f"    {n.get('at')} {n.get('kind')}" + (f" ({n['by']})" if n.get("by") else "") + f": {n.get('text')}")
    elif task.get("last_failure"):
        failure = task["last_failure"]
        lines.append(f"  last failure: {failure.get('at')}: {failure.get('reason')}")
    return "\n".join(lines)


# ==== cli/query.py ====
# How `atm project status` prints atm-lib's status report (docs/layered-architecture.md §9, step 3).
#
# Text output is short and stable, for agents to read; --json is for anything that parses it (CLI-3.3).

def show_status(status, as_json):
    """Print atm-lib's status report (`atm project status`)."""
    result = status["report"]
    if as_json:
        out(to_json(result))
        return
    sprint, stranded = result["sprint"], result.get("open_in_closed_sprints")
    pairs = lambda d: ", ".join(f"{k} {v}" for k, v in d.items())
    elsewhere = lambda i: f" [in another worktree, {i['branch'] or 'detached'}]" if "branch" in i else ""
    if sprint:
        details = [f"{sprint['done']} of {sprint['tasks']} done"] + [
            f"{k} {sprint[k]}" for k in ("due", "release") if sprint.get(k)
        ]
        out(f"sprint: {sprint['id']} (active): {' · '.join(details)}")
        if sprint.get("goal"):
            out(f"  goal: {sprint['goal']}")
        if sprint["review_pending"]:
            out(f"  waiting for the sprint review: {', '.join(sprint['review_pending'])} (/atm:close-sprint)")
    elif status["has_sprints"]:
        out("sprint: none active (plan one with /atm:plan-sprint)")
    if stranded:
        n = len(stranded)
        out(
            f"warning: {n} open task{'' if n == 1 else 's'} in closed sprints, which the loop won't take: "
            + ", ".join(f"{t['id']} ({t['sprint']})" for t in stranded)
            + " (atm sprint move <id> --to backlog or --to active)"
        )
    out(f"{result['tasks']} tasks: {pairs(result['by_status'])}")
    if status["has_sprints"]:
        out(f"by sprint: {pairs(result['by_sprint'])}")
    out(f"by type: {pairs(result['by_type'])}")
    if result["by_label"]:
        out(f"by label: {pairs(result['by_label'])}")
    for title, items, extra in (
        ("in progress", result["in_progress"], lambda i: f" ({i['by']})" if i["by"] else ""),
        ("in review", result["in_review"], lambda i: (f" (since {i['since']})" if i["since"] else "") + elsewhere(i)),
        ("blocked", result["blocked"], lambda i: (f" (on {i['waiting_on']})" if i["waiting_on"] else "") + elsewhere(i)),
        ("waiting on dependencies", result["waiting_on_dependencies"], lambda i: f" (on {', '.join(i['on'])})"),
    ):
        if items:
            out(f"{title}: " + "; ".join(f"{i['id']} {i['name']}{extra(i)}" for i in items))
    nxt = result["next"]
    out(f"next: {nxt['id']} {nxt['name']}" if nxt else f"next: none ({result['next_reason']})")
    largest = ", ".join(f"{i['id']} {i['bytes'] // 1024 or 1} KB" for i in result["largest_tasks"])
    out(f"file: {result['file_bytes'] // 1024} KB" + (f"; largest tasks: {largest}" if largest else ""))
    if result["archived"]:
        out(f"archive: {result['archived']} tasks, {result['archive_bytes'] // 1024} KB")
    hint = (result.get("suggest") or {}).get("archive")
    if hint:
        out(hint["text"])


# ==== cli/status.py ====
# What `atm task state` passes to atm-lib's status functions (docs/layered-architecture.md §9, step 8).

# Each status function's options, as atm-lib names them (§5.1): (parameter, the command's option)
COMMAND_OPTIONS = {
    "start": (),
    "review": (("note", "note"),),
    "done": (("note", "note"), ("commit", "commit"), ("resolution", "resolution")),
    "reject": (("reason", "reason"),),
    "approve": (("note", "note"),),
    "request-changes": (("reason", "reason"),),
    "block": (("on", "on"), ("questions", "question"), ("reason", "reason")),
    "unblock": (("answers", "answer"),),
    "reopen": (("reason", "reason"),),
    "skip": (("reason", "reason"), ("resolution", "resolution"), ("duplicate_of", "duplicate_of")),
    "fail": (("reason", "reason"),),
    "release": (("note", "note"),),
}
LIST_OPTIONS = ("questions", "answers")


# ==== cli/claims.py ====
# What the claim commands read from a hook (`atm claim heartbeat --auto`, the plugin's PostToolUse hook).

def hook_input():
    """The JSON a Claude Code hook gets on stdin ({session_id, cwd, ...}), or {}."""
    if sys.stdin is None or sys.stdin.isatty():
        return {}
    try:
        value = json.loads(sys.stdin.read() or "{}")
    except ValueError:
        return {}
    return value if isinstance(value, dict) else {}


# ==== cli/edit.py ====
# What `atm task add` and `task edit` read and print beyond atm-lib (docs/layered-architecture.md §9, step 10): the
# file --json-file names (atm-lib takes its content), and the components a task's files suggest.

def suggestion_line(vocab, task):
    """`atm task add` / `task edit --add-file` (SR-2.7): the components the task's files suggest that it doesn't have
    yet, as a line with the command that sets them; None when there are none (or the project has no components)."""
    missing = [c for c in suggest_components(vocab, task.get("files") or []) if c not in (task.get("components") or [])]
    if not missing:
        return None
    adds = " ".join(f"--add-component {c}" for c in missing)
    return f"  {task['id']}: its files suggest the component{'s' if len(missing) > 1 else ''} {', '.join(missing)} (atm task edit {task['id']} {adds})"


def read_entries(path):
    """The tasks of an `atm task add --json-file`: a list of task objects, or {"tasks": [...]}."""
    value = read_json(path)
    if isinstance(value, dict):
        value = value.get("tasks")
    if not isinstance(value, list) or not all(isinstance(e, dict) for e in value):
        raise Invalid(f"{path}: expected a JSON list of tasks (or {{\"tasks\": [...]}})")
    return value


# ==== cli/components.py ====
# How `atm component add` and `component edit` describe what they moved (docs/layered-architecture.md §9, step 10).

def moved_text(label, from_label, assign, by_files):
    """What --from-label and --assign did, e.g. "6 tasks from the label atm-cli, 2 more by their files"."""
    parts = []
    if label:
        parts.append(f"{len(from_label)} task{'' if len(from_label) == 1 else 's'} from the label {label}")
    if assign:
        parts.append(f"{len(by_files)} {'more ' if label else ''}by their files")
    return ", ".join(parts)


# ==== cli/sprints.py ====
# How the `atm sprint` commands print sprints (docs/layered-architecture.md §9, step 11).

NONE_ACTIVE = "No sprint is active now: start the next one (atm sprint start <name>), or plan one with /atm:plan-sprint."


def describe_sprint(s):
    details = [s["state"]] + [f"{key} {s[key]}" for key in ("release", "due") if s.get(key)]
    counts = ", ".join(f"{kind} {n}" for kind, n in s["by_kind"].items())
    line = f"{s['id']} · {' · '.join(details)}: {s['tasks']} task{'' if s['tasks'] == 1 else 's'}"
    line += f" ({counts})" if counts else ""
    if s.get("review_pending"):
        line += f"; {s['review_pending']} waiting for the sprint review"
    return line + (f"\n  goal: {s['goal']}" if s.get("goal") else "")


def show_sprints(project, include_closed, as_json):
    listed = list_sprints(project, include_closed)
    sprints, backlog = listed["sprints"], listed["backlog"]
    if as_json:
        out(to_json({"sprints": sprints, "backlog": backlog}))
        return
    if not sprints:
        out("No open sprints (--all lists the closed ones)." if listed["has_sprints"] else "No sprints yet: atm sprint add creates one.")
    for s in sprints:
        out(describe_sprint(s))
    out(f"backlog: {backlog} task{'' if backlog == 1 else 's'} (atm sprint backlog)")


def show_close(result, as_json):
    sprint, created, target = result["sprint"], result["created"], result["target"]
    if as_json:
        out(to_json({"sprint": sprint, "created": created}))
        return
    report = sprint["report"]
    line = f"Closed the sprint “{sprint['id']}”: {len(report['done'])} done"
    if report["carried"]:
        where = "the backlog" if target is None else f"“{target}”" + (" (new, planned)" if created else "")
        line += f", {len(report['carried'])} carried to {where} ({', '.join(report['carried'])})"
    if report["added"]:
        line += f", {len(report['added'])} added mid-sprint"
    out(line + ".")
    if result["was_active"]:
        out(NONE_ACTIVE)


def show_delete(result, as_json):
    sprint, state, moved, target = result["sprint"], result["state"], result["moved"], result["target"]
    if as_json:
        out(to_json({"deleted": sprint["id"], "state": state, "moved": moved, "to": target or "backlog"}))
        return
    line = f"Deleted the sprint “{sprint['id']}” ({state})"
    if moved:
        where = "the backlog" if target is None else f"“{target}”"
        line += f": {len(moved)} task{'' if len(moved) == 1 else 's'} moved to {where} ({', '.join(moved)})"
    out(line + ".")
    if state == "active":
        out(NONE_ACTIVE)


# ==== cli/releases.py ====
# How `atm release list` and `release notes` print releases (docs/layered-architecture.md §9, step 11).

def describe_release(r):
    details = [r["state"]] + ([f"target {r['target']}"] if r.get("target") else [])
    if r.get("released_at"):
        details.append(f"released {r['released_at'][:10]}")
    if isinstance(r.get("pr"), dict) and r["pr"].get("number"):
        details.append(f"PR #{r['pr']['number']}")
    title = r["id"] + (f" “{r['name']}”" if r.get("name") else "")
    kinds = ", ".join(f"{k} {n}" for k, n in r["by_kind"].items())
    stages = ", ".join(f"{s.replace('_', ' ')} {n}" for s, n in r["by_stage"].items())
    line = f"{title} · {' · '.join(details)}: {r['tasks']} task{'' if r['tasks'] == 1 else 's'}"
    return line + (f" ({kinds})" if kinds else "") + (f"; {stages}" if stages else "")


def describe_notes(notes):
    """The draft as Markdown: a heading per type, a line per task; what's not done yet at the end."""
    release = notes["release"]
    lines = [f"# {release['id']}" + (f" “{release['name']}”" if release.get("name") else "")]
    if not notes["groups"]:
        lines += ["", "Nothing done in it yet."]
    for group in notes["groups"]:
        lines += ["", f"## {group['label']}", ""]
        for t in group["tasks"]:
            lines.append(f"- **{t['id']}** {t['name']}" + (f": {t['outcome']}" if t["outcome"] else ""))
    if notes["open"]:
        lines += ["", "## Not done yet", ""]
        lines += [f"- {t['id']} {t['name']} ({t['status']})" for t in notes["open"]]
    return lines


# ==== cli/archive.py ====
# How `atm db archive` and `db unarchive` describe the tasks they moved (docs/layered-architecture.md §9, step 12).

def describe_ids(ids, limit=12):
    return ", ".join(ids[:limit]) + (f" and {len(ids) - limit} more" if len(ids) > limit else "")


def describe_kept(kept):
    """ "T-010 (in open PR #1); T-011, T-012 (finished in the last 14 days)": grouped by reason, in first-seen order"""
    groups = {}
    for k in kept:
        groups.setdefault(k["reason"], []).append(k["id"])
    return "; ".join(f"{describe_ids(ids)} ({reason})" for reason, ids in groups.items())


# ==== cli/code.py ====
# How `atm project workflow` describes the project's workflow (docs/layered-architecture.md §9, step 13).

def describe_workflow(workflow, base_default):
    if workflow is None:
        return "No workflow set: /atm:push asks for one, or run `atm project workflow set direct|pull-request`."
    trailers = workflow.get("trailers")
    lines = [
        f"workflow: {workflow['type']}",
        "  base: " + (workflow["base"] if workflow.get("base") else f"{base_default()} (the remote's default branch)"),
        f"  verify before pushing: {'yes' if workflow['verify'] else 'no'}",
        f"  CI fix rounds: {workflow['max_rounds']}",
        f"  commit message: {workflow['commit_template']}",
        "  trailers: "
        + (
            "none (Co-Authored-By lines are removed)"
            if trailers == "forbid"
            else ("required: " + "; ".join(trailers["require"]) if isinstance(trailers, dict) else "as the agent writes them")
        ),
    ]
    return "\n".join(lines)


# ==== cli/projects.py ====
# `atm project list|add|remove|prune`: the projects the dashboard shows, through atm-lib.

def describe_entry(entry):
    return f"{entry['id']}  {entry['name']}  {entry['path']}"


def damaged_notice(damaged):
    return f"projects.json was unreadable: moved it to {damaged} and started an empty registry." if damaged else None


def projects_action(registry, action, target, as_json):
    """List, add, remove or prune the dashboard's projects (`atm project list|add|remove|prune`)."""
    if action == "add":
        result = add_project(registry, target)
        if as_json:
            out(to_json(result))
            return
        if result["damaged"]:
            out(damaged_notice(result["damaged"]))
        out(f"{'Added' if result['result'] == 'added' else 'Already registered'}: {describe_entry(result['project'])}")
    elif action == "remove":
        result = remove_project(registry, target)
        if as_json:
            out(to_json(result))
            return
        out(f"Removed: {describe_entry(result['removed'])}")
    elif action == "prune":
        result = prune_projects(registry)
        if as_json:
            out(to_json(result))
            return
        if result["damaged"]:
            out(damaged_notice(result["damaged"]))
        if not result["removed"]:
            out("Nothing to prune: every registered project is there.")
        for e in result["removed"]:
            out(f"Removed: {describe_entry(e)}")
    else:
        if target:
            raise Invalid(f"unexpected argument '{target}'")
        result = list_projects(registry)
        if as_json:
            out(to_json(result))
            return
        if result["damaged"]:
            out(damaged_notice(result["damaged"]))
        if not result["projects"]:
            out("No projects registered. `atm project add [PATH]` adds one; atm adds a project when it runs there.")
        for e in result["projects"]:
            out(describe_entry(e) + ("  (not found)" if e["missing"] else ""))


# ==== cli/settings.py ====
# `atm project logo`: the project's logo, through atm-lib. Setting the store itself up (`atm db init`: the merge
# driver, the dashboard's list) is boot's, the one place that knows the store.

def logo_action(project, action, path, as_json):
    """Print, set or unset the logo (`atm project logo`)."""
    if action is None:
        if path is not None:
            raise Invalid(f"unexpected argument '{path}' (atm project logo set PATH)")
        info = get_logo(project)
        logo, chosen = info["found"], info["set"]
        if as_json:
            found = {"logo": logo["path"], "source": logo["source"]} if logo else {"logo": None, "source": None}
            out(to_json({**found, "set": chosen}))
        elif logo:
            out(logo["path"] + (" (found; set one with `atm project logo set <path>`)" if logo["source"] == "found" else ""))
        elif chosen:
            out(f"meta.logo is {chosen}, which isn't a usable logo (missing, too large, or outside the project).")
        else:
            out("No logo. Set one with `atm project logo set <path>` (SVG, PNG, WebP or ICO, inside the project).")
        return
    if action == "unset":
        removed = unset_logo(project)
        out(to_json({"logo": None}) if as_json else "Logo unset." if removed else "No logo was set.")
        return
    if path is None:
        raise Invalid("atm project logo set needs a path")
    # The path is relative to where atm runs, like every path atm takes; atm-lib keeps it relative to the project
    relative = set_logo(project, path)
    out(to_json({"logo": relative}) if as_json else f"Logo set: {relative}")


# ==== cli/store.py ====
# How `atm project config` prints where the task files are. The commands about the JSON-in-git store itself (`atm db
# migrate`, `db merge-driver`, the lock test; docs/layered-architecture.md §3.2, §11.2) reach it only through
# `boot/jsongit.py`: a store that isn't files in git has none of them.

def show_paths(paths, as_json):
    """Print where the task files are (`atm project config`)."""
    if as_json:
        out(to_json(paths))
        return
    for key in ["root", "dir", "tasks", "dashboard"]:
        out(f"{key}: {paths[key]}")
    if paths["legacy"]:
        out(f"legacy: {paths['legacy']['tasks']}")


# ==== cli/serve.py ====
# `atm serve`: the CLI's side of the dashboard server, one per user, a detached process serving every registered project
# (docs/multi-project-api.md). The CLI starts, stops, checks and refreshes the server as a process (its record in the
# user's atm folder, its pid); the server itself is the REST API's, run by `plugin/scripts/atm-serve` (the built
# atm.py entered as the server). The two never import each other (docs/layered-architecture.md §11.1).

# Held while one `atm serve` checks for a server and starts one, so two at once (two sessions' hooks) start one (T-390)
SERVER_LOCK = "server.lock"
SERVER_LOCK_WAIT = 15.0
STOP_WAIT = 5.0
KILL_WAIT = 2.0


def server_url(port, project=None):
    return f"http://localhost:{port}/" + (f"#project={project}" if project else "")


def live_server(home):
    """The record of the user's running server, or None. A stale record is removed."""
    record_path = os.path.join(home, SERVER_RECORD)
    record = read_json(record_path)
    if record is None:
        return None
    info = whoami(record.get("port"))
    if info and info.get("pid") == record.get("pid"):
        return record
    os.remove(record_path)
    return None


def open_browser(url):
    opener = "open" if sys.platform == "darwin" else "xdg-open"
    try:
        subprocess.Popen([opener, url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        out(f"Could not open a browser; open {url} yourself.")


@contextlib.contextmanager
def server_lock(home):
    """Only one `atm serve` at a time looks for a running server and starts one: a directory, which mkdir creates for
    exactly one process. A lock whose holder is gone (its pid isn't running) or that outlived the wait is broken."""
    ensure_home(home)
    lock = os.path.join(home, SERVER_LOCK)
    deadline = time.time() + SERVER_LOCK_WAIT
    while True:
        try:
            os.mkdir(lock)
            break
        except FileExistsError:
            holder = read_json(os.path.join(lock, "pid.json")) or {}
            gone = isinstance(holder.get("pid"), int) and not pid_alive(holder["pid"])
            if gone or time.time() >= deadline:
                shutil.rmtree(lock, ignore_errors=True)
                continue
            time.sleep(0.05)
    try:
        with open(os.path.join(lock, "pid.json"), "w", encoding="utf-8") as f:
            f.write(to_json({"pid": os.getpid()}))
        yield
    finally:
        shutil.rmtree(lock, ignore_errors=True)


def ensure_server(home, port):
    """The running server's record, starting the server process if none runs. Returns (record, started). Under the
    server lock, so two at once start one server, and the second gets the first's."""
    with server_lock(home):
        record = live_server(home)
        if record:
            return record, False
        return start_server(home, port), True


def start_server(home, port):
    """Start the server process and wait for its record."""
    with open(os.path.join(home, SERVER_LOG), "ab") as log:
        child = subprocess.Popen(
            server_command(home, port),
            cwd=home,
            stdin=subprocess.DEVNULL,
            stdout=log,
            stderr=log,
            start_new_session=True,
        )
    deadline = time.time() + 10
    while time.time() < deadline:
        record = read_json(os.path.join(home, SERVER_RECORD))
        if record and record.get("pid") == child.pid and whoami(record["port"]):
            return record
        if child.poll() is not None:
            break
        time.sleep(0.05)
    raise AtmFailure(f"the dashboard server did not start; see {os.path.join(home, SERVER_LOG)}")


def serve_start(cwd, port, overview):
    """Start (or reuse) the server; returns the URL to show: the project's (at or above `cwd`), or the overview's."""
    project = None
    if not overview:
        migration = migrate_folder(cwd, register_it=False)
        for line in migration["lines"]:
            if line != "Nothing to migrate.":
                out(line)
        if not migration["tasks_exists"]:
            raise AtmFailure("no tasks.json to serve. Run /atm:create-tasks first.")
        project, shown = dashboard_id(open_project(cwd))
        if not shown:
            out("This project isn't listed on the dashboard (meta.dashboard.listed is false).")
            project = None
    home = atm_home()
    record, started = ensure_server(home, port)
    url = server_url(record["port"], project)
    out(f"Dashboard server {'started' if started else 'already running'}: {url}")
    return url


def wait_gone(pid, seconds):
    deadline = time.time() + seconds
    while time.time() < deadline and pid_alive(pid):
        time.sleep(0.05)
    return not pid_alive(pid)


def stop_server(home, pid):
    """Stop a server of the user's (whose pid whoami confirmed): SIGTERM, then SIGKILL if it outlives the wait. Its
    record goes only once it is gone, and only if it is that server's: a server still running is never forgotten
    (T-390)."""
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    if not wait_gone(pid, STOP_WAIT):
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        if not wait_gone(pid, KILL_WAIT):
            raise AtmFailure(f"the dashboard server (pid {pid}) didn't stop; its record is kept")
    record_path = os.path.join(home, SERVER_RECORD)
    if (read_json(record_path) or {}).get("pid") == pid:
        os.remove(record_path)


def unrecorded_servers(home, first_port, record=None):
    """The servers of this atm folder answering on the usual ports (from `first_port`) that the record doesn't name: a
    server whose record was lost or overwritten (T-390). A server of another folder, or one too old to say its
    folder, isn't this folder's to stop."""
    mine = home_id(home)
    found = []
    for port in range(first_port, first_port + SCAN_PORTS):
        info = whoami(port, timeout=0.5)
        if info and info.get("home") == mine and info.get("pid") != (record or {}).get("pid"):
            found.append(info)
    return found


def serve_stop(home, port):
    record_path = os.path.join(home, SERVER_RECORD)
    record = read_json(record_path)
    live = bool(record) and (whoami(record.get("port")) or {}).get("pid") == record.get("pid")
    strays = unrecorded_servers(home, port, record if live else None)
    if record is None and not strays:
        out("Dashboard server is not running.")
        return
    if record is not None and not live:
        os.remove(record_path)
        if not strays:
            out(f"Dashboard server is not running (removed a stale record for pid {record.get('pid')}).")
            return
    if live:
        stop_server(home, record["pid"])
        out(f"Dashboard server stopped (pid {record['pid']}).")
    for info in strays:
        stop_server(home, info["pid"])
        out(f"Stopped a dashboard server the record didn't name (pid {info['pid']}, port {info['port']}).")


def outdated(record):
    """True when the server runs an older dashboard than this plugin's: an older version, or the same version but
    another build of this plugin's own dashboard file (it was rebuilt since the server started). A newer server is
    left alone, so a session still on an older plugin never downgrades it. Another install of the same version (a
    development checkout, a worktree, the plugin cache) is left alone too: neither build is newer, and replacing it
    would make each install restart the server in turn, every open page offering a reload each time."""
    mine, mine_build = dashboard_head(TEMPLATE) or ("", "")
    theirs, theirs_build = str(record.get("version") or ""), str(record.get("build") or "")
    if parse_version(mine) != parse_version(theirs):
        return parse_version(mine) > parse_version(theirs)
    theirs_root = record.get("plugin_root")
    if theirs_root and os.path.realpath(str(theirs_root)) != os.path.realpath(PLUGIN_ROOT):
        return False
    return bool(mine_build) and mine_build != theirs_build


def refresh_server(home=None):
    """Restart the user's server on the same port if it runs an older dashboard than this plugin's (after a plugin
    update). Cheap when there is nothing to do: one read of server.json, and no request unless it is outdated.
    Never starts a server that isn't running. Returns the new record, or None."""
    home = home or atm_home()
    record = read_json(os.path.join(home, SERVER_RECORD))
    if not isinstance(record, dict) or not outdated(record):
        return None
    with server_lock(home):
        live = live_server(home)
        if not live:
            return None
        stop_server(home, live["pid"])
        return start_server(home, live["port"])


def serve_refresh(home):
    record = refresh_server(home)
    if record:
        out(f"Dashboard server restarted with dashboard {record.get('version')}: {server_url(record['port'])}")


def serve_status(home, as_json, port):
    record = live_server(home)
    strays = unrecorded_servers(home, port, record)
    if as_json:
        url = server_url(record["port"]) if record else None
        unrecorded = [{"pid": s["pid"], "port": s["port"], "version": s.get("version")} for s in strays]
        out(to_json({"running": bool(record), **(record or {}), "url": url, "unrecorded": unrecorded}))
        return
    if not record:
        out("Dashboard server is not running.")
    else:
        up = format_duration((datetime.now(timezone.utc) - parse_iso(record["started_at"])).total_seconds())
        out(
            f"Dashboard server running: {server_url(record['port'])} "
            f"(pid {record['pid']}, dashboard {record.get('version')}, up {up})"
        )
    for info in strays:
        out(
            f"Also running, without a record: pid {info['pid']} on port {info['port']} "
            f"(dashboard {info.get('version')}); `atm service stop` stops it."
        )


# ==== cli/main.py ====
# The command line: atm's command tree, in groups (task, loop, claim, vcs, release, component, project, sprint; db,
# doctor and service for maintenance), built with Typer on Click (vendored in plugin/scripts/vendor/), and main().
# Design: docs/improving-atm-cli-ergonomics.md.
#
# `--json` and `--dry-run` (`-n`) go anywhere on the line; `--dir PATH` first runs as if started in PATH. Errors are
# `atm: <message>` on stderr, or {"error": "...", "code": N} with --json; a usage error has exit code 2.

# `atm --help` lists the commands people and agents use first (task, loop, claim, vcs, release, component, project,
# sprint), then the maintenance ones, then help and version; what git, hooks and the server call is hidden
# (db merge-driver, project summary). The panels are rich's, which isn't vendored (plugin/scripts/vendor/README.md):
# plain Click help shows the order only.
MAINTENANCE = "Maintenance"
META = "Meta"
HELP_ORDER = ["task", "loop", "claim", "vcs", "release", "component", "project", "sprint", "db", "doctor", "service",
              "help", "version"]


class OrderedGroup(typer.core.TyperGroup):
    """The top-level commands in HELP_ORDER (Typer lists commands before groups)."""

    def list_commands(self, ctx):
        names = super().list_commands(ctx)
        return sorted(names, key=lambda n: HELP_ORDER.index(n) if n in HELP_ORDER else len(HELP_ORDER))


app = typer.Typer(
    name="atm",
    help=(
        "atm, the Agentic Task Manager's command line: its commands, by group. "
        "`atm <command> --help` describes one.\n\n"
        "\b\n"
        "Every command takes --json (-j) and --dry-run (-n), anywhere on the line.\n"
        "atm --dir PATH <command> runs as if started in PATH."
    ),
    no_args_is_help=True,
    cls=OrderedGroup,
)


@app.callback()
def _global(
    ctx: typer.Context,
    dry_run: bool = typer.Option(
        False, "--dry-run", "-n", help="Don't execute, just show what would be done"
    ),
    json_out: bool = typer.Option(False, "--json", "-j", help="Print result as JSON"),
) -> None:
    """Global options for every command (they may go anywhere on the line)."""
    ctx.obj = {"dry_run": dry_run, "json_out": json_out}


# ---------------------------------------------------------------------------
# task group
# ---------------------------------------------------------------------------

task_app = typer.Typer(help="Task lifecycle commands")
app.add_typer(task_app, name="task")


@task_app.command("list")
def task_list(
    ctx: typer.Context,
    status: Optional[list[str]] = typer.Option(
        None, "--status", help="Only these statuses (repeatable)"
    ),
    sprint: Optional[list[str]] = typer.Option(
        None, "--sprint", help="Only this sprint's tasks (repeatable)"
    ),
    backlog: bool = typer.Option(False, "--backlog", help="Only tasks with no sprint"),
    release: Optional[str] = typer.Option(None, "--release", help="Only tasks in this release"),
    component: Optional[str] = typer.Option(
        None, "--component", help="Only tasks in this component"
    ),
    type_: Optional[str] = typer.Option(None, "--type", help="Only this task type"),
    priority: Optional[str] = typer.Option(None, "--priority", help="Only this priority"),
    label: Optional[list[str]] = typer.Option(
        None, "--label", help="Only tasks with this label (repeatable)"
    ),
    agent: Optional[str] = typer.Option(None, "--agent", help="Only tasks started by this agent"),
    ready: bool = typer.Option(
        False, "--ready", help="Only to-do tasks whose dependencies are done"
    ),
    blocked: bool = typer.Option(
        False, "--blocked", help="Only blocked tasks and to-do tasks still waiting"
    ),
    in_pr: bool = typer.Option(
        False, "--in-pr", help="Only tasks in a PR that isn't recorded as merged"
    ),
    with_user: bool = typer.Option(
        False, "--with-user", help="Only tasks done together with the user"
    ),
    archived: bool = typer.Option(False, "--archived", help="List the archive instead"),
    derived: bool = typer.Option(False, "--derived", help="Each task in full as JSON"),
    tolerant: bool = typer.Option(
        False, "--tolerant", help="With --derived: read a malformed tasks.json as far as it can be read"
    ),
) -> None:
    """List tasks: one line per task: ID, status, priority, labels, name."""
    filters = Filters(
        sprint=sprint or None,
        backlog=backlog,
        release=release,
        component=component,
        status=status or None,
        type=type_,
        priority=priority,
        label=label or None,
        agent=agent,
        ready=ready,
        blocked=blocked,
        in_pr=in_pr,
        with_user=with_user,
        archived=archived,
    )
    if derived:
        # JSON either way: it is for tools (the dashboard's tests take atm's own values from it)
        if tolerant:
            snapshot = task_snapshot(open_project(os.getcwd(), tolerant=True), filters)
            snapshot.pop("bytes")
            out(to_json(snapshot))
        else:
            out(to_json(task_views(open_project(os.getcwd()), filters)))
        return
    rows = [view_row(v) for v in task_views(open_project(os.getcwd()), filters)]
    if ctx.obj.get("json_out", False):
        out(to_json(rows))
        return
    if not rows:
        out("No tasks match.")
        return
    for line in list_lines(rows):
        out(line)


@task_app.command("add")
def task_add(
    ctx: typer.Context,
    name: Optional[str] = typer.Option(None, "--name", help="A short name (about 60 characters)"),
    type_: Optional[str] = typer.Option(None, "--type", help="The task type"),
    sprint: Optional[str] = typer.Option(None, "--sprint", help="The sprint it goes into"),
    release: Optional[str] = typer.Option(None, "--release", help="The release it ships in"),
    priority: Optional[str] = typer.Option(None, "--priority", help="The priority"),
    label: Optional[list[str]] = typer.Option(None, "--label", help="A label (repeatable)"),
    description: Optional[str] = typer.Option(None, "--description", help="What exactly to do"),
    description_file: Optional[str] = typer.Option(
        None, "--description-file", help="Read the description from a file"
    ),
    dep: Optional[list[str]] = typer.Option(
        None, "--dep", help="A task it depends on (repeatable)"
    ),
    file: Optional[list[str]] = typer.Option(
        None, "--file", help="A file it creates or changes (repeatable)"
    ),
    requirement: Optional[list[str]] = typer.Option(
        None, "--requirement", help="A requirement (repeatable)"
    ),
    component: Optional[list[str]] = typer.Option(
        None, "--component", help="A component (repeatable)"
    ),
    components_auto: Optional[str] = typer.Option(
        None,
        "--components",
        click_type=click.Choice(["auto"]),
        help="auto: the components its files are in (otherwise only suggested)",
    ),
    review: Optional[str] = typer.Option(None, "--review", help="How it is reviewed"),
    related: Optional[list[str]] = typer.Option(
        None, "--related", help="A related task (repeatable)"
    ),
    with_user: Optional[str] = typer.Option(
        None, "--with", help="user: done together with the user"
    ),
    json_file: Optional[str] = typer.Option(None, "--json-file", help="Add many from a JSON file"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Add a task with the next ID, or many from a JSON file."""
    if description and description_file:
        raise Invalid("--description and --description-file can't both be given")
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would add tasks from {json_file}" if json_file else f"[dry-run] would add task {name}")
        return
    if json_file:
        entries = read_entries(json_file)
    else:
        if description_file:
            with open(description_file, encoding="utf-8") as f:
                description = f.read()
        entries = [
            {
                "name": name,
                "type": type_,
                "priority": priority,
                "labels": label or [],
                "description": description or "",
                "dependencies": dep or [],
                "files": file or [],
                "requirements": requirement or [],
                "sprint": sprint,
                "release": release,
                "components": "auto" if components_auto else (component or []),
                "review": review,
                "with": with_user,
                "related": related or [],
            }
        ]
    result = add_new_tasks(open_project(os.getcwd(), agent_identity(agent)), entries)
    if ctx.obj.get("json_out", False):
        out(to_json(result["tasks"]))
        return
    for task in result["tasks"]:
        out(f"Added {task['id']} {task['name']}")
        line = suggestion_line(result["vocab"], task)
        if line:
            out(line)


@task_app.command("edit")
def task_edit(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    name: Optional[str] = typer.Option(None, "--name"),
    description: Optional[str] = typer.Option(None, "--description"),
    description_file: Optional[str] = typer.Option(None, "--description-file"),
    sprint: Optional[str] = typer.Option(None, "--sprint"),
    backlog: bool = typer.Option(False, "--backlog"),
    type_: Optional[str] = typer.Option(None, "--type"),
    priority: Optional[str] = typer.Option(None, "--priority"),
    release: Optional[str] = typer.Option(None, "--release"),
    review: Optional[str] = typer.Option(None, "--review"),
    add_file: Optional[list[str]] = typer.Option(None, "--add-file"),
    remove_file: Optional[list[str]] = typer.Option(None, "--remove-file"),
    add_dep: Optional[list[str]] = typer.Option(None, "--add-dep"),
    remove_dep: Optional[list[str]] = typer.Option(None, "--remove-dep"),
    add_label: Optional[list[str]] = typer.Option(None, "--add-label"),
    remove_label: Optional[list[str]] = typer.Option(None, "--remove-label"),
    add_component: Optional[list[str]] = typer.Option(None, "--add-component"),
    remove_component: Optional[list[str]] = typer.Option(None, "--remove-component"),
    add_related: Optional[list[str]] = typer.Option(None, "--add-related"),
    remove_related: Optional[list[str]] = typer.Option(None, "--remove-related"),
    add_requirement: Optional[list[str]] = typer.Option(None, "--add-requirement"),
    remove_requirement: Optional[list[str]] = typer.Option(None, "--remove-requirement"),
    add_use_case: Optional[list[str]] = typer.Option(None, "--add-use-case"),
    remove_use_case: Optional[list[str]] = typer.Option(None, "--remove-use-case"),
    components_auto: Optional[str] = typer.Option(
        None,
        "--components",
        click_type=click.Choice(["auto"]),
        help="auto: add the components its files are in (otherwise only suggested)",
    ),
    with_user: Optional[str] = typer.Option(None, "--with"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Change a task's definition (not its status: use `atm task state`)."""
    if description is not None and description_file:
        raise Invalid("--description and --description-file can't both be given")
    if sprint is not None and backlog:
        raise Invalid("--sprint and --backlog can't both be given")
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would edit task {task_id}")
        return
    if description_file:
        with open(description_file, encoding="utf-8") as f:
            description = f.read()
    result = edit_existing_task(
        open_project(os.getcwd(), agent_identity(agent)),
        task_id,
        name=name,
        description=description,
        sprint=sprint,
        backlog=backlog,
        task_type=type_,
        priority=priority,
        release=release,
        review=review,
        with_=with_user,
        components="auto" if components_auto else None,
        add_files=add_file or [],
        remove_files=remove_file or [],
        add_dependencies=add_dep or [],
        remove_dependencies=remove_dep or [],
        add_labels=add_label or [],
        remove_labels=remove_label or [],
        add_components=add_component or [],
        remove_components=remove_component or [],
        add_related=add_related or [],
        remove_related=remove_related or [],
        add_requirements=add_requirement or [],
        remove_requirements=remove_requirement or [],
        add_use_cases=add_use_case or [],
        remove_use_cases=remove_use_case or [],
    )
    task, changed = result["task"], result["changed"]
    if ctx.obj.get("json_out", False):
        out(to_json(task))
        return
    out(f"{task['id']}: changed {', '.join(changed)}" if changed else f"{task['id']}: nothing changed")
    # New files may put the task in components it isn't in yet (SR-2.7): suggested, not set
    line = suggestion_line(result["vocab"], task) if add_file else None
    if line:
        out(line)


@task_app.command("details")
def task_details(
    ctx: typer.Context,
    task_ids: Annotated[list[str], typer.Argument(help="Task IDs")],
) -> None:
    """Tasks in full (description, dependencies, files, notes)."""
    views = show_tasks(open_project(os.getcwd()), task_ids)
    if ctx.obj.get("json_out", False):
        # An archived task says so (T-353)
        out(to_json([{**v["task"], "archived": True} if v["archived"] else v["task"] for v in views]))
    else:
        out("\n\n".join(render_task(v) for v in views))


@task_app.command("note")
def task_note(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    text: Annotated[str, typer.Argument(help="The note text")],
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Add a note to a task (at most 500 characters)."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would add a note to {task_id}")
        return
    task = add_task_note(open_project(os.getcwd(), agent_identity(agent)), task_id, text)
    if ctx.obj.get("json_out", False):
        out(to_json(task["notes"][-1]))
    else:
        out(f"{task['id']}: note added ({len(task['notes'])} in all)")


@task_app.command("deps")
def task_deps(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
) -> None:
    """What a task waits for, and what waits for it."""
    deps = task_dependencies(open_project(os.getcwd()), task_id)
    if ctx.obj.get("json_out", False):
        out(to_json(deps))
        return
    line = lambda items: ", ".join(f"{i['id']} ({i['status']})" for i in items) or "nothing"
    out(f"{task_id} waits for: {line(deps['waits_for'])}")
    out(f"{task_id} is waited on by: {line(deps['waited_on_by'])}")


@task_app.command("state")
def task_state(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    next_state: Annotated[
        str, typer.Argument(help="Next state: todo, in_progress, in_review, done, blocked, skipped")
    ],
    note: Optional[str] = typer.Option(None, "--note", help="A note"),
    reason: Optional[str] = typer.Option(None, "--reason", help="A reason"),
    commit: Optional[str] = typer.Option(None, "--commit", help="The task's commit SHA"),
    resolution: Optional[str] = typer.Option(None, "--resolution", help="How it ended"),
    duplicate_of: Optional[str] = typer.Option(
        None, "--duplicate-of", help="The task it duplicates"
    ),
    on: Optional[str] = typer.Option(
        None, "--on", click_type=click.Choice(["user", "agent"]), help="blocked: who has to decide"
    ),
    question: Optional[list[str]] = typer.Option(
        None, "--question", help="An open question (repeatable)"
    ),
    answer: Optional[list[str]] = typer.Option(None, "--answer", help="An answer (repeatable)"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who runs it"),
) -> None:
    """Move a task to another state: in_progress, in_review, done, todo, blocked or skipped. A review's decision is
    `atm task review`."""
    # The state -> atm-lib's status function
    cmd_map = {
        "in_progress": "start",
        "in_review": "review",
        "done": "done",
        "todo": "fail",  # chosen below by the kind the task is in now
        "blocked": "block",
        "skipped": "skip",
    }
    cmd = cmd_map.get(next_state)
    if cmd is None:
        raise Invalid(f"unknown state '{next_state}'. Use: {', '.join(sorted(cmd_map))}")

    project = open_project(os.getcwd(), agent_identity(agent))
    if next_state == "todo":
        # Back to todo is a different status function for each kind the task is in (a failure says why: --reason)
        cmd = todo_command(project, task_id, failing=reason is not None)

    given = {
        "note": note,
        "reason": reason,
        "commit": commit,
        "resolution": resolution,
        "duplicate_of": duplicate_of,
        "on": on,
        "question": question,
        "answer": answer,
    }
    takes = {name for _, name in COMMAND_OPTIONS[cmd]}
    extra = [name for name, value in given.items() if value and name not in takes]
    if extra:
        flags = ", ".join("--" + name.replace("_", "-") for name in extra)
        raise Invalid(f"{flags}: not for {next_state} ({cmd}) from where {task_id} is")
    required = {"reject": "reason", "reopen": "reason", "skip": "reason", "block": "on"}
    if cmd in required and not given[required[cmd]]:
        raise Invalid(f"{next_state} ({cmd}) needs --{required[cmd]}")
    run_status(ctx, project, cmd, task_id, status, given)


def run_status(ctx, project, cmd, task_id, status, given):
    """Run atm-lib's status function `cmd` with the options `given`, and print the result (task state, task review)."""
    if ctx.obj.get("dry_run", False):
        out(f"[dry-run] would {cmd} {task_id}")
        return
    options = {
        parameter: given.get(name) or ([] if parameter in LIST_OPTIONS else None)
        for parameter, name in COMMAND_OPTIONS[cmd]
    }
    result = STATUS_FUNCTIONS[cmd](project, task_id, status=status, **options)
    report_expired(result["expired"])
    task, hint, stranded = result["task"], result["hint"], result["stranded"]
    if ctx.obj.get("json_out", False):
        extra_fields = {"unblocked": result["unblocked"]} if cmd == "done" else {}
        if hint:
            extra_fields["suggest"] = {"archive": hint}
        if stranded:
            extra_fields["hint"] = stranded
        out(to_json({**task, "claim": result["claim"], **extra_fields}))
        return
    out(f"{task['id']}: {result['previous']} -> {task['status']}")
    if stranded:
        out(f"  {stranded}")
    if cmd == "start":
        out(render_task(result["view"]))
    if result["unblocked"]:
        out("Now ready: " + ", ".join(result["unblocked"]))
    if hint:
        out(hint["text"])


# A review's decisions: the task stays in review (T-438 adds reject and guards the exits from review). The decision
# comes first, so a permission rule can tell them apart; no skill pre-approves them (I23)
review_app = typer.Typer(help="Decide a task's review: approve it or ask for changes; it stays in review")
task_app.add_typer(review_app, name="review")


@review_app.command("approve")
def review_approve(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    note: Optional[str] = typer.Option(None, "--note", help="What is left to do, or a word for the agent"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who decides"),
) -> None:
    """Approve a task in review: it stays in review until an agent takes it back to finish."""
    run_status(ctx, open_project(os.getcwd(), agent_identity(agent)), "approve", task_id, status, {"note": note})


@review_app.command("request-changes")
def review_request_changes(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    reason: str = typer.Option(..., "--reason", help="What needs to change (a rejection note)"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who decides"),
) -> None:
    """Ask for changes to a task in review: its agent takes it back first."""
    run_status(
        ctx, open_project(os.getcwd(), agent_identity(agent)), "request-changes", task_id, status, {"reason": reason}
    )


# ---------------------------------------------------------------------------
# loop group
# ---------------------------------------------------------------------------

loop_app = typer.Typer(help="The work loop")
app.add_typer(loop_app, name="loop")


@loop_app.command("next")
def loop_next(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(
        None, "--sprint", help="This sprint's tasks (repeatable; default: the active one)"
    ),
    tasks: Optional[str] = typer.Option(None, "--tasks", metavar="ID,ID", help="These tasks, in this order"),
    all_tasks: bool = typer.Option(
        False, "--all", help="Every task: the active sprint, the planned ones, the backlog"
    ),
    backlog: bool = typer.Option(False, "--backlog", help="The tasks with no sprint"),
    type_: Optional[str] = typer.Option(None, "--type", help="Only this task type"),
    with_user: bool = typer.Option(
        False, "--with-user", help="Also tasks done together with the user (the user is watching)"
    ),
    claim: bool = typer.Option(False, "--claim", help="Also start and claim it, atomically"),
    status: Optional[str] = typer.Option(None, "--status", help="Pick among same-kind statuses"),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who you are"),
) -> None:
    """The task /atm:loop-tasks works on next (exit code 3 if there is none)."""
    given = [flag for flag, value in (("--sprint", sprint), ("--tasks", tasks), ("--all", all_tasks), ("--backlog", backlog)) if value]
    if len(given) > 1:
        raise Invalid(f"{' and '.join(given)} can't both be given: they choose the tasks in different ways")
    if ctx.obj.get("dry_run", False):
        out(f"[dry-run] would {'claim' if claim else 'show'} the next task")
        return
    sel = selection(
        sprints=sprint or None,
        tasks=[i.strip() for i in (tasks or "").split(",") if i.strip()],
        all_tasks=all_tasks,
        backlog=backlog,
        task_type=type_,
    )
    project = open_project(os.getcwd(), agent_identity(agent))
    result = next_task_for(project, sel, with_user=with_user, claim=claim, status=status)
    report_expired(result["expired"])
    task, claimed, reason = result["task"], result["claim"], result["reason"]
    if ctx.obj.get("json_out", False):
        out(to_json({"task": task, "claim": claimed, "reason": reason}))
    elif task:
        verb = "Claimed" if claimed else "Next"
        where = task.get("sprint") or "backlog"
        out(f"{verb}: {task['id']} {task.get('name', '')} ({where}, {task.get('type')})")
    else:
        out(f"No task available: {reason}.")
    if not task:
        raise AtmExit(3)


@loop_app.command("run")
def loop_run(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    cmd_and_args: Annotated[
        Optional[list[str]], typer.Argument(help="The command and its args, after --: -- npm test --verbose")
    ] = None,
    every: Optional[float] = typer.Option(
        None, "--every", metavar="SECONDS", help="Renew this often (default: three times per lease, at least once a minute)"
    ),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who you are"),
) -> None:
    """Run a long command while keeping your claim on a task alive; exits with the command's exit code."""
    if not cmd_and_args:
        raise Invalid("atm loop run needs a command after --: atm loop run <id> -- <command> [args...]")
    if every is not None and every <= 0:
        raise Invalid("--every must be more than 0 seconds")
    if ctx.obj.get("dry_run", False):
        out(f"[dry-run] would run {' '.join(cmd_and_args)} while keeping the claim on {task_id}")
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    renew = lambda: renew_lease(open_project(os.getcwd(), project.actor), task_id, expire=False)
    minutes = renew()["minutes"]
    interval = every if every is not None else max(60.0, minutes * 60 / 3)
    try:
        child = subprocess.Popen(cmd_and_args)
    except OSError as e:
        raise AtmFailure(f"can't run {cmd_and_args[0]}: {e.strerror}", 127)
    # Renewed from this (the main) thread between waits: the lock uses signals, which only work here
    renewing = True
    while True:
        try:
            code = child.wait(timeout=interval)
            break
        except subprocess.TimeoutExpired:
            pass
        except KeyboardInterrupt:  # the command got the same interrupt; wait for it to end
            continue
        if renewing:
            try:
                renew()
            except Exception as e:  # never stops the command; say so once and stop renewing
                sys.stderr.write(f"atm loop run: stopped renewing the claim on {task_id}: {e}\n")
                renewing = False
    if code:
        raise AtmExit(128 - code if code < 0 else code)  # a signal -N ends like a shell's 128+N


# ---------------------------------------------------------------------------
# claim group
# ---------------------------------------------------------------------------

claim_app = typer.Typer(help="Claim lifecycle")
app.add_typer(claim_app, name="claim")


@claim_app.command("list")
def claim_list(
    ctx: typer.Context,
) -> None:
    """List live claims, shared by all worktrees of this clone."""
    live =expire_claims(open_project(os.getcwd()))["claims"]
    if ctx.obj.get("json_out", False):
        out(to_json(live))
        return
    if not live:
        out("No live claims.")
        return
    current = now()
    for c in live:
        left = format_duration((parse_iso(c["lease_expires_at"]) - current).total_seconds())
        out(f"{c['task']}  {c.get('agent')}  {c.get('branch') or '-'}  {left} left  ({c.get('worktree')})")


def _auto_heartbeat(agent):
    """`heartbeat --auto`, run by the plugin's PostToolUse hook after every tool call (CLI-6.2): renew this agent's
    live claims once their last renewal is ten minutes old. It never prints and never fails, so a tool call is never
    disturbed."""
    try:
        payload = hook_input()
        cwd = payload.get("cwd") if isinstance(payload.get("cwd"), str) and os.path.isdir(payload["cwd"]) else os.getcwd()
        if payload.get("session_id") and not (agent or os.environ.get("ATM_AGENT")):
            # The same identity atm commands get in that folder: a subagent in a linked worktree renews its own claims
            actor = session_identity(payload["session_id"], cwd)
        else:
            actor = agent_identity(agent)
        project = hook_project(cwd, actor)  # also lists the project on the dashboard
        if project is not None:
            renew_due_claims(project)
    except Exception:  # a hook must never break the tool call it follows
        return


@claim_app.command("heartbeat")
def claim_heartbeat(
    ctx: typer.Context,
    task_id: Optional[str] = typer.Argument(None, help="The task ID"),
    auto: bool = typer.Option(
        False, "--auto", help="Renew your claims if their last renewal is ten minutes old; silent (the plugin's hook)"
    ),
    agent: Optional[str] = typer.Option(None, "--agent", help="Who you are"),
) -> None:
    """Extend your claim's lease."""
    if ctx.obj.get("dry_run", False):
        out("[dry-run] would renew claims due for it" if auto else f"[dry-run] would extend the lease on {task_id}")
        return
    if auto:
        _auto_heartbeat(agent)
        return
    if not task_id:
        raise Invalid("atm claim heartbeat needs a task ID (or --auto)")
    claim = renew_lease(open_project(os.getcwd(), agent_identity(agent)), task_id)["claim"]
    if ctx.obj.get("json_out", False):
        out(to_json(claim))
    else:
        out(f"{task_id}: lease extended to {claim['lease_expires_at']}")


# ---------------------------------------------------------------------------
# vcs group
# ---------------------------------------------------------------------------

vcs_app = typer.Typer(help="Version control integration")
app.add_typer(vcs_app, name="vcs")


@vcs_app.command("commit")
def vcs_commit(
    ctx: typer.Context,
    task_id: Annotated[str, typer.Argument(help="The task ID")],
    file: Optional[list[str]] = typer.Option(
        None, "--file", help="A file the task changed (repeatable)"
    ),
    dir_: Optional[list[str]] = typer.Option(
        None, "--dir", help="Every change under a folder (repeatable)"
    ),
    message: Optional[str] = typer.Option(None, "--message", help="The commit message"),
    record: Optional[str] = typer.Option(
        None, "--record", help="Record this existing commit instead"
    ),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Commit exactly the named files plus tasks.json, and record the commit."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [f"atm vcs commit {task_id}"]
        if file:
            for f in file:
                parts.append(f"--file {f}")
        if dir_:
            for d in dir_:
                parts.append(f"--dir {d}")
        if message:
            parts.append(f"--message {message}")
        if record:
            parts.append(f"--record {record}")
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    if record is not None:
        if file or dir_ or message is not None:
            raise Invalid("--record records an existing commit: no --file, --dir or --message")
        result = record_commit(project, task_id, record)
        if ctx.obj.get("json_out", False):
            shown = {k: result[k] for k in ("task", "commit", "recorded")}
            out(to_json({**shown, **({"hint": result["hint"]} if result["hint"] else {})}))
            return
        out(f"{result['task']}: recorded {result['commit'][:7]} {result['summary']}".rstrip())
        if result["hint"]:
            out(f"  {result['hint']}")
        return
    files = repository_paths(project, file or [], os.getcwd())
    if dir_:
        files += [f for f in folder_changes(project, task_id, dir_, os.getcwd()) if f not in files]
    result = commit_task(project, task_id, files, message)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    out(f"{result['task']}: committed {result['commit'][:7]} {result['message'].splitlines()[0]}")
    out("  " + ", ".join(result["files"]))


@vcs_app.command("pr")
def vcs_pr(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
    number: int = typer.Option(..., "--number", help="The PR number"),
    url: str = typer.Option(..., "--url", help="The PR's URL"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Record the PR that contains the tasks."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [f"atm vcs pr {' '.join(ids)} --number {number} --url {url}"]
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    task_ids = record_pr(project, ids, number, url)
    if ctx.obj.get("json_out", False):
        out(to_json({"tasks": task_ids, "pr": {"number": number, "url": url}}))
    else:
        out(f"{', '.join(task_ids)}: in PR #{number} ({url})")


@vcs_app.command("pushed")
def vcs_pushed(
    ctx: typer.Context,
    pr: int = typer.Option(..., "--pr", help="The PR number"),
    at: Optional[str] = typer.Option(None, "--at", help="When it was merged (ISO 8601 UTC)"),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Record that a PR was merged, on the tasks it contains."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [f"atm vcs pushed --pr {pr}"]
        if at:
            parts.append(f"--at {at}")
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    result = record_merge(project, pr, at)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    task_ids = result["tasks"]
    out(f"PR #{pr} merged: {', '.join(task_ids)}" if task_ids else f"PR #{pr}: already recorded as merged")
    for release_id in result["released"]:
        out(f"Released {release_id}: all its tasks are merged in PR #{pr}.")
    for release_id, waiting in result["pending"].items():
        out(f"{release_id} stays planned: {', '.join(waiting)} {'is' if len(waiting) == 1 else 'are'} not merged in PR #{pr}.")


@vcs_app.command("repository")
def vcs_repository(
    ctx: typer.Context,
    url: Optional[str] = typer.Argument(None, help="An http(s) URL or git remote"),
    old: Optional[str] = typer.Option(
        None, "--from", help="Move recorded PR links from this address"
    ),
    agent: Optional[str] = typer.Option(None, "--agent"),
) -> None:
    """Print or set the repository's web address."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = ["atm vcs repository"]
        if url:
            parts.append(url)
        if old:
            parts.append(f"--from {old}")
        if agent:
            parts.append(f"--agent {agent}")
        out("[dry-run] would run: " + " ".join(parts))
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    if url is None:
        if old is not None:
            raise Invalid(f"unexpected argument '{old}' (atm vcs repository URL)")
        current = get_repository(project)
        if ctx.obj.get("json_out", False):
            out(to_json({"repository": current}))
        elif current:
            out(current)
        else:
            out("No repository address yet. Set it with `atm vcs repository <url>` (a git remote works too).")
        return
    result = set_repository(project, url, old)
    moved = result["moved_links"]
    if ctx.obj.get("json_out", False):
        out(to_json({"repository": result["repository"], "moved_links": moved}))
    else:
        links = f" ({moved} PR link{'' if moved == 1 else 's'} moved from {result['previous']})" if moved else ""
        out(f"Repository address set: {result['repository']}{links}")


# ---------------------------------------------------------------------------
# db group
# ---------------------------------------------------------------------------

db_app = typer.Typer(help="Database / file maintenance")
app.add_typer(db_app, name="db", rich_help_panel=MAINTENANCE)


@db_app.command("migrate")
def db_migrate(
    ctx: typer.Context,
) -> None:
    """Move tasks.json into .agentic-task-manager/ and set up the folder."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out("[dry-run] would move tasks.json into .agentic-task-manager/ and set up the folder")
        return
    migration = migrate_folder(os.getcwd())
    if ctx.obj.get("json_out", False):
        out(to_json(migration["result"]))
    else:
        for line in migration["lines"]:
            out(line)


@db_app.command("merge-driver", hidden=True)
def db_merge_driver(
    ctx: typer.Context,
    base: Annotated[str, typer.Argument(help="The common ancestor's tasks.json")],
    ours: Annotated[str, typer.Argument(help="This branch's tasks.json (written with the result)")],
    theirs: Annotated[str, typer.Argument(help="The other branch's tasks.json")],
) -> None:
    """git's merge driver for tasks.json (git runs it; `db init` registers it)."""
    conflicts = merge_versions(base, ours, theirs)
    if ctx.obj.get("json_out", False):
        out(to_json({"conflicts": conflicts}))
    if conflicts:
        sys.stderr.write("atm: tasks.json has conflicting edits (this branch's version was kept):\n")
        for conflict in conflicts:
            sys.stderr.write(f"  {conflict}\n")
        raise AtmExit(1)


@db_app.command("archive")
def db_archive(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(None, "--sprint"),
    before: Optional[str] = typer.Option(None, "--before"),
    keep_days: Optional[float] = typer.Option(None, "--keep-days"),
) -> None:
    """Move finished tasks to tasks-archive.json (with --dry-run: say which, and move nothing)."""
    # atm-lib's own dry run says which tasks would move
    dry_run = ctx.obj.get("dry_run", False)
    result = archive_finished(
        open_project(os.getcwd()), sprint, before, keep_days, dry_run
    )
    ids, dropped, kept, sizes = (
        result["moved"],
        result["duplicates_dropped"],
        result["kept"],
        result["sizes"],
    )
    shown = {
        "moved": ids,
        "dry_run": bool(dry_run),
        "tasks_kb": sizes["tasks"] // 1024,
        "archive_kb": sizes["archive"] // 1024,
        "duplicates_dropped": dropped,
        "kept": kept,
    }
    if ctx.obj.get("json_out", False):
        out(to_json(shown))
        return
    if dropped and not dry_run:
        out(
            f"Kept {describe_ids(dropped)} in tasks.json and dropped the archive's copy (an interrupted move)."
        )
    if not ids:
        out("Nothing to archive.")
    elif dry_run:
        out(f"Would archive {len(ids)} task(s): {describe_ids(ids)}.")
    else:
        out(
            f"Archived {len(ids)} task(s): {describe_ids(ids)}. "
            f"tasks.json is now {shown['tasks_kb']} KB, tasks-archive.json {shown['archive_kb']} KB."
        )
    if kept:
        out(
            f"{'Would keep' if dry_run else 'Kept'} {len(kept)} finished: {describe_kept(kept)}."
        )


@db_app.command("unarchive")
def db_unarchive(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
) -> None:
    """Bring archived tasks back into tasks.json."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would unarchive {', '.join(ids)}")
        return
    result = unarchive_tasks(open_project(os.getcwd()), ids)
    again = result["moves_back"]
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    out(f"Brought back {', '.join(ids)} from the archive.")
    if again:
        out(
            f"{describe_ids(again)} {'is' if len(again) == 1 else 'are'} finished: the next `atm db archive` moves "
            f"{'it' if len(again) == 1 else 'them'} back, unless reopened (`atm task state <id> todo --reason …`)."
        )


@db_app.command("init")
def db_init(
    ctx: typer.Context,
    meta_file: Optional[str] = typer.Option(
        None, "--meta-file", help="Set tasks.json meta from a JSON object"
    ),
) -> None:
    """Give tasks.json a meta.id and register the merge driver."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out("[dry-run] would give tasks.json a meta.id and register the merge driver")
        return
    project = open_project(os.getcwd())
    if meta_file:
        new = read_json(meta_file)
        if not isinstance(new, dict):
            raise Invalid(f"{meta_file} must hold a JSON object: the meta keys to set")
        created, keys = init_meta(project, new)
        project = open_project(os.getcwd())  # the task data as it is now
        if not ctx.obj.get("json_out", False):
            out(("Created tasks.json; meta: " if created else "Updated tasks.json meta: ") + (", ".join(keys) or "-"))
    project_id = ensure_project_id(project)
    repository = ensure_repository(project)
    register_project(project)  # the dashboard lists every project atm runs in
    setup = set_up_store(project)
    if ctx.obj.get("json_out", False):
        out(
            to_json(
                {
                    "project_id": project_id,
                    "repository": repository,
                    "git": setup["git"],
                    "gitattributes_added": setup["gitattributes_added"],
                }
            )
        )
        return
    if project_id:
        out(f"Gave this project an id ({project_id}) in tasks.json meta, for the dashboard's saved view.")
    if repository:
        out(f"Recorded the repository address ({repository}), for commit and PR links on the dashboard.")
    if not setup["git"]:
        out("Not in a git repository: there is no merge driver to set up.")
        return
    out(
        "Registered the tasks.json merge driver (git config merge.atm)"
        + (
            " and added .agentic-task-manager/.gitattributes (staged)."
            if setup["gitattributes_added"]
            else "; .gitattributes already set."
        )
    )


# ---------------------------------------------------------------------------
# release group
# ---------------------------------------------------------------------------

release_app = typer.Typer(help="Release management")
app.add_typer(release_app, name="release")


@release_app.command("list")
def release_list(
    ctx: typer.Context,
    all_: bool = typer.Option(False, "--all", help="List archived releases too"),
) -> None:
    """List releases with their tasks by kind and stage."""
    project = open_project(os.getcwd())
    listed = list_releases(project, all_)
    if ctx.obj.get("json_out", False):
        out(to_json(listed["releases"]))
        return
    if not listed["releases"]:
        out(
            "No open releases (--all lists the archived ones)."
            if listed["has_releases"]
            else "No releases yet: atm release add <R>."
        )
        return
    for r in listed["releases"]:
        out(describe_release(r))


@release_app.command("add")
def release_add(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
    name: Optional[str] = typer.Option(None, "--name", help="Add, edit: a name besides its ID (empty: remove it)"),
    target: Optional[str] = typer.Option(None, "--target", metavar="DATE", help="Add, edit: the planned date, YYYY-MM-DD"),
    pr: Optional[list[int]] = typer.Option(None, "--pr", metavar="N", help="Add: give it the tasks of pull request N (repeatable)"),
    released_at: Optional[str] = typer.Option(None, "--released-at", metavar="TIME", help="Add: it shipped already, at this time (e.g. its PR's merge)"),
) -> None:
    """Add a release."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [release_id]
        if name:
            parts.append(f"--name {name}")
        if target:
            parts.append(f"--target {target}")
        if pr:
            for n in pr:
                parts.append(f"--pr {n}")
        if released_at:
            parts.append(f"--released-at {released_at}")
        out(f"[dry-run] would add release {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = add_release(
        project, release_id, name=name, target=target, prs=pr or [], released_at=released_at
    )
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    release, given, prs = result["release"], result["tasks"], pr or []
    line = f'Added the release “{release_id}” ({release["state"]})'
    if given:
        numbers = ", ".join(f"#{n}" for n in prs)
        line += f", with the {len(given)} task{'' if len(given) == 1 else 's'} of PR{'s' if len(prs) > 1 else ''} {numbers}"
    out(line + ".")


@release_app.command("edit")
def release_edit(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
    name: Optional[str] = typer.Option(None, "--name", help="Add, edit: a name besides its ID (empty: remove it)"),
    target: Optional[str] = typer.Option(None, "--target", metavar="DATE", help="Add, edit: the planned date, YYYY-MM-DD"),
) -> None:
    """Edit a release."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [release_id]
        if name is not None:
            parts.append(f"--name {name}")
        if target is not None:
            parts.append(f"--target {target}")
        out(f"[dry-run] would edit release {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = edit_release(project, release_id, name=name, target=target)
    release, changed = result["release"], result["changed"]
    if ctx.obj.get("json_out", False):
        out(to_json(release))
    else:
        out(f"{release['id']}: changed {', '.join(changed)}" if changed else f"{release['id']}: nothing changed")


@release_app.command("archive")
def release_archive(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
) -> None:
    """Archive a release."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        out(f"[dry-run] would archive release {release_id}")
        return
    project = open_project(os.getcwd())
    release = archive_release(project, release_id)
    if ctx.obj.get("json_out", False):
        out(to_json(release))
    else:
        out(f"Archived the release “{release['id']}”.")


@release_app.command("notes")
def release_notes_cmd(
    ctx: typer.Context,
    release_id: Annotated[str, typer.Argument(help="The release's ID (letters, digits, '.', '-', '_')")],
) -> None:
    """Print a Markdown draft of a release's changes."""
    project = open_project(os.getcwd())
    notes = release_notes(project, release_id)
    if ctx.obj.get("json_out", False):
        out(to_json(notes))
        return
    for line in describe_notes(notes):
        out(line)


# ---------------------------------------------------------------------------
# component group
# ---------------------------------------------------------------------------

component_app = typer.Typer(help="Component management")
app.add_typer(component_app, name="component")


@component_app.command("list")
def component_list(
    ctx: typer.Context,
) -> None:
    """List components with their task counts."""
    project = open_project(os.getcwd())
    listed = list_components(project)
    if ctx.obj.get("json_out", False):
        out(to_json(listed))
        return
    if not listed:
        out("No components yet: atm component add <id> --path GLOB.")
        return
    for c in listed:
        details = [f"“{c['label']}”"] if c.get("label") else []
        details.append(", ".join(c["paths"]) if c["paths"] else "no paths")
        if c.get("review"):
            details.append(f"review {c['review']}")
        out(f"{c['id']} · {' · '.join(details)}: {c['tasks']} task{'' if c['tasks'] == 1 else 's'}")


@component_app.command("add")
def component_add(
    ctx: typer.Context,
    component_id: Annotated[str, typer.Argument(help="The component's ID (letters, digits, '.', '-', '_')")],
    label: Optional[str] = typer.Option(None, "--label", help="Add, edit: the name shown"),
    path: Optional[list[str]] = typer.Option(None, "--path", metavar="GLOB", help="Add: a glob over its files, e.g. 'dashboard/**' (repeatable)"),
    review: Optional[str] = typer.Option(None, "--review", help="Add, edit: how its tasks are reviewed"),
    from_label: Optional[str] = typer.Option(None, "--from-label", metavar="LABEL", help="Add, edit: every task with this label gets the component instead"),
    assign: bool = typer.Option(False, "--assign", help="Add, edit: also give it to every task whose files its globs match"),
) -> None:
    """Add a component."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [component_id]
        if label:
            parts.append(f"--label {label}")
        if path:
            for p in path:
                parts.append(f"--path {p}")
        if review:
            parts.append(f"--review {review}")
        if from_label:
            parts.append(f"--from-label {from_label}")
        if assign:
            parts.append("--assign")
        out(f"[dry-run] would add component {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = add_component(project, component_id, label, path, review, from_label, assign)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    text = moved_text(from_label, result["from_label"], assign, result["by_files"])
    out(f"Added the component {component_id}" + (f": {text}." if text else "."))


@component_app.command("edit")
def component_edit(
    ctx: typer.Context,
    component_id: Annotated[str, typer.Argument(help="The component's ID (letters, digits, '.', '-', '_')")],
    label: Optional[str] = typer.Option(None, "--label", help="Add, edit: the name shown (add: default the label's own text, with --from-label; edit: empty removes it)"),
    add_path: Optional[list[str]] = typer.Option(None, "--add-path", metavar="GLOB", help="Edit: add a glob over its files (repeatable)"),
    review: Optional[str] = typer.Option(None, "--review", help="Add, edit: how its tasks are reviewed (blocker, async, batch, none; edit: empty removes it)"),
    from_label: Optional[str] = typer.Option(None, "--from-label", metavar="LABEL", help="Add, edit: every task with this label gets the component instead"),
    assign: bool = typer.Option(False, "--assign", help="Add, edit: also give it to every task whose files its globs match"),
) -> None:
    """Edit a component."""
    dry_run = ctx.obj.get("dry_run", False)
    if dry_run:
        parts = [component_id]
        if label is not None:
            parts.append(f"--label {label}")
        if add_path:
            for p in add_path:
                parts.append(f"--add-path {p}")
        if review is not None:
            parts.append(f"--review {review}")
        if from_label:
            parts.append(f"--from-label {from_label}")
        if assign:
            parts.append("--assign")
        out(f"[dry-run] would edit component {' '.join(parts)}")
        return
    project = open_project(os.getcwd())
    result = edit_component(project, component_id, label, add_path, review, from_label, assign)
    if ctx.obj.get("json_out", False):
        out(to_json(result))
        return
    moved = moved_text(from_label, result["from_label"], assign, result["by_files"])
    text = ", ".join(([f"changed {', '.join(result['changed'])}"] if result["changed"] else []) + [t for t in [moved] if t])
    out(f"{component_id}: {text or 'nothing changed'}")


# ---------------------------------------------------------------------------
# doctor / service
# ---------------------------------------------------------------------------

project_app = typer.Typer(help="Project-level config")
app.add_typer(project_app, name="project")


def _choice(name, value, choices):
    """A value that must be one of `choices` (or not given), refused as atm's argparse refuses it (exit code 2)."""
    if value is not None and value not in choices:
        raise Invalid(f"{name}: invalid choice '{value}' (choose from {', '.join(choices)})")
    return value


def _dry(ctx, what):
    """With --dry-run, say what would be done and stop; otherwise go on."""
    if ctx.obj.get("dry_run", False):
        out(f"[dry-run] would {what}")
        return True
    return False


@project_app.command("config")
def project_config(
    ctx: typer.Context,
) -> None:
    """Print where this project's task files are."""
    show_paths(task_paths(os.getcwd()), ctx.obj.get("json_out", False))


@project_app.command("summary", hidden=True)
def project_summary_cmd(ctx: typer.Context) -> None:
    """The project's numbers as atm serve gives them to the dashboard (JSON; the dashboard's dev server reads it)."""
    out(to_json(project_summary(open_project(os.getcwd()))))


@project_app.command("status")
def project_status(
    ctx: typer.Context,
    sprint: Optional[list[str]] = typer.Option(None, "--sprint", help="Only this sprint's tasks (repeatable)"),
    backlog: bool = typer.Option(False, "--backlog", help="Only the tasks with no sprint"),
    release: Optional[str] = typer.Option(None, "--release", help="Only the tasks in this release"),
    component: Optional[str] = typer.Option(None, "--component", help="Only the tasks in this component"),
) -> None:
    """The overview: counts, what is in progress, in review or blocked, the next task, the file's size."""
    status = status_report(
        open_project(os.getcwd()),
        Filters(sprint=sprint or None, backlog=backlog, release=release, component=component),
    )
    show_status(status, ctx.obj.get("json_out", False))


@project_app.command("workflow")
def project_workflow(
    ctx: typer.Context,
    action: Optional[str] = typer.Argument(None, help="set: change the workflow"),
    wf_type: Optional[str] = typer.Argument(None, help="direct or pull-request"),
    base: Optional[str] = typer.Option(None, "--base", help="The branch PRs target, or that direct pushes to"),
    verify: Optional[str] = typer.Option(None, "--verify", help="Run the project's verify before pushing (true|false)"),
    max_rounds: Optional[int] = typer.Option(None, "--max-rounds", help="CI fix rounds before /atm:push stops"),
    commit_template: Optional[str] = typer.Option(
        None, "--commit-template", metavar="TEXT", help='Commit messages, e.g. "feat({id}): {name}"'
    ),
    trailers: Optional[str] = typer.Option(
        None, "--trailers", help="forbid Co-Authored-By lines, require the --trailer lines, or none (the agent's)"
    ),
    trailer: Optional[list[str]] = typer.Option(None, "--trailer", metavar="LINE", help="A required trailer line"),
) -> None:
    """Print the project's workflow, or set it (atm project workflow set TYPE)."""
    _choice("action", action, ["set"])
    _choice("--verify", verify, ["true", "false"])
    _choice("--trailers", trailers, ["forbid", "require", "none"])
    project = open_project(os.getcwd())
    if action is None:
        workflow = get_workflow(project)
    else:
        if _dry(ctx, f"set the workflow to {wf_type}"):
            return
        workflow = set_workflow(
            project,
            wf_type,
            base=base,
            verify=None if verify is None else verify == "true",
            max_rounds=max_rounds,
            commit_template=commit_template,
            trailers=trailers,
            trailer=trailer or [],
        )
    if ctx.obj.get("json_out", False):
        out(to_json(workflow or {"type": None}))
    else:
        out(describe_workflow(workflow, project.vcs.default_base))


@project_app.command("logo")
def project_logo_cmd(
    ctx: typer.Context,
    action: Optional[str] = typer.Argument(None, help="set or unset it"),
    path: Optional[str] = typer.Argument(None, help="set: the image (SVG, PNG, WebP or ICO), relative to here"),
) -> None:
    """Print, set or unset the project's logo for the dashboard (meta.logo)."""
    _choice("action", action, ["set", "unset"])
    if action is not None and _dry(ctx, f"{action} the logo"):
        return
    logo_action(open_project(os.getcwd()), action, path, ctx.obj.get("json_out", False))


@project_app.command("list")
def project_list(ctx: typer.Context) -> None:
    """List the projects the dashboard shows."""
    projects_action(open_registry(), "list", None, ctx.obj.get("json_out", False))


@project_app.command("add")
def project_add(
    ctx: typer.Context,
    target: Optional[str] = typer.Argument(None, help="The project's folder (default: here)"),
) -> None:
    """Add a project to the dashboard's list."""
    if _dry(ctx, f"add {target or 'this folder'} to the dashboard's projects"):
        return
    projects_action(open_registry(), "add", target, ctx.obj.get("json_out", False))


@project_app.command("remove")
def project_remove(
    ctx: typer.Context,
    target: Annotated[str, typer.Argument(help="The project's ID or path")],
) -> None:
    """Remove a project from the dashboard's list."""
    if _dry(ctx, f"remove {target} from the dashboard's projects"):
        return
    projects_action(open_registry(), "remove", target, ctx.obj.get("json_out", False))


@project_app.command("prune")
def project_prune(ctx: typer.Context) -> None:
    """Drop the projects whose folder is gone from the dashboard's list."""
    if _dry(ctx, "drop the projects whose folder is gone"):
        return
    projects_action(open_registry(), "prune", None, ctx.obj.get("json_out", False))


# ---------------------------------------------------------------------------
# sprint group
# ---------------------------------------------------------------------------

sprint_app = typer.Typer(help="Sprint management")
app.add_typer(sprint_app, name="sprint")


@sprint_app.command("list")
def sprint_list(
    ctx: typer.Context,
    all_: bool = typer.Option(False, "--all", help="List closed sprints too"),
) -> None:
    """List the sprints with their task counts, and the backlog's size."""
    show_sprints(open_project(os.getcwd()), all_, ctx.obj.get("json_out", False))


AGENT_HELP = "Who you are (default: ATM_AGENT, else this session)"


@sprint_app.command("add")
def sprint_add(
    ctx: typer.Context,
    name: Optional[str] = typer.Argument(None, help="The sprint's name (generated when left out)"),
    goal: Optional[str] = typer.Option(None, "--goal", help="What the sprint is for"),
    release: Optional[str] = typer.Option(None, "--release", help="The release it delivers to"),
    due: Optional[str] = typer.Option(None, "--due", metavar="DATE", help="The planned end, YYYY-MM-DD"),
    after: Optional[str] = typer.Option(None, "--after", metavar="NAME", help="Put it after this sprint (default: last)"),
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Add a sprint (planned)."""
    if _dry(ctx, f"add the sprint {name or '(a generated name)'}"):
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    sprint = add_sprint(project, name, goal=goal, release=release, due=due, after=after)
    out(to_json(sprint) if ctx.obj.get("json_out", False) else f"Added the sprint “{sprint['id']}” (planned).")


@sprint_app.command("edit")
def sprint_edit(
    ctx: typer.Context,
    name: Annotated[str, typer.Argument(help="The sprint's name")],
    goal: Optional[str] = typer.Option(None, "--goal", help="What the sprint is for (empty: remove it)"),
    release: Optional[str] = typer.Option(None, "--release", help="The release it delivers to (empty: remove it)"),
    due: Optional[str] = typer.Option(None, "--due", metavar="DATE", help="The planned end, YYYY-MM-DD (empty: remove it)"),
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Change a sprint's goal, release or due date."""
    if _dry(ctx, f"edit the sprint {name}"):
        return
    result = edit_sprint(open_project(os.getcwd(), agent_identity(agent)), name, goal=goal, release=release, due=due)
    sprint, changed = result["sprint"], result["changed"]
    if ctx.obj.get("json_out", False):
        out(to_json(sprint))
    else:
        out(f"{sprint['id']}: changed {', '.join(changed)}" if changed else f"{sprint['id']}: nothing changed")


@sprint_app.command("start")
def sprint_start(
    ctx: typer.Context,
    name: Annotated[str, typer.Argument(help="The sprint's name")],
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Make a planned sprint the active one."""
    if _dry(ctx, f"start the sprint {name}"):
        return
    sprint = start_sprint(open_project(os.getcwd(), agent_identity(agent)), name)
    out(to_json(sprint) if ctx.obj.get("json_out", False) else f"The sprint “{sprint['id']}” is now active.")


@sprint_app.command("close")
def sprint_close(
    ctx: typer.Context,
    name: Annotated[str, typer.Argument(help="The sprint's name")],
    carry_to: Optional[str] = typer.Option(
        None, "--carry-to", metavar="WHERE", help="Where unfinished tasks go: a sprint's name, new, or backlog"
    ),
    new_name: Optional[str] = typer.Option(
        None, "--name", metavar="NAME", help="With --carry-to new: the new sprint's name (default: generated)"
    ),
    reviewed: bool = typer.Option(
        False, "--reviewed", help="Its batch tasks were reviewed with the user (the sprint review)"
    ),
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Close a sprint: record its report and move its unfinished tasks on."""
    if _dry(ctx, f"close the sprint {name}"):
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    closed = close_sprint(project, name, carry_to=carry_to, new_name=new_name, reviewed=reviewed)
    show_close(closed, ctx.obj.get("json_out", False))


@sprint_app.command("delete")
def sprint_delete(
    ctx: typer.Context,
    name: Annotated[str, typer.Argument(help="The sprint's name")],
    move_to: Optional[str] = typer.Option(
        None, "--move-to", metavar="WHERE", help="Where its tasks go: a sprint's name or backlog (default)"
    ),
    force: bool = typer.Option(False, "--force", help="Also a sprint tasks finished in or moved on from"),
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Delete a sprint created by mistake."""
    if _dry(ctx, f"delete the sprint {name}"):
        return
    project = open_project(os.getcwd(), agent_identity(agent))
    show_delete(delete_sprint(project, name, move_to=move_to, force=force), ctx.obj.get("json_out", False))


@sprint_app.command("move")
def sprint_move(
    ctx: typer.Context,
    ids: Annotated[list[str], typer.Argument(help="Task IDs")],
    to: str = typer.Option(..., "--to", metavar="WHERE", help="A sprint's name, next, active, or backlog"),
    agent: Optional[str] = typer.Option(None, "--agent", help=AGENT_HELP),
) -> None:
    """Move tasks into a sprint, the next sprint, the active one, or the backlog."""
    if _dry(ctx, f"move {', '.join(ids)} to {to}"):
        return
    moved = move_tasks(open_project(os.getcwd(), agent_identity(agent)), ids, to)
    if ctx.obj.get("json_out", False):
        out(to_json(moved))
        return
    for m in moved:
        before, after = m["from"] or "backlog", m["to"] or "backlog"
        out(f"{m['id']}: {before} -> {after}" if before != after else f"{m['id']}: already in {after}")


@sprint_app.command("backlog")
def sprint_backlog(
    ctx: typer.Context,
    all_: bool = typer.Option(False, "--all", help="Also the finished tasks with no sprint"),
) -> None:
    """The tasks with no sprint (not finished ones, unless --all)."""
    found = backlog_tasks(open_project(os.getcwd()), finished=all_)
    rows = [summary(found["vocab"], t) for t in found["tasks"]]
    if ctx.obj.get("json_out", False):
        out(to_json(rows))
    elif not rows:
        out("The backlog is empty.")
    else:
        for line in list_lines(rows):
            out(line)


# ---------------------------------------------------------------------------
# release group
# ---------------------------------------------------------------------------
# doctor / service
# ---------------------------------------------------------------------------


@app.command("doctor", rich_help_panel=MAINTENANCE)
def doctor(
    ctx: typer.Context,
    files: bool = typer.Option(False, "--files", help="Also warn about missing files"),
) -> None:
    """Validate tasks.json: schema, vocabularies, dependencies."""
    result = check_project(open_project(os.getcwd()), files=files)
    errors, warnings = result["errors"], result["warnings"]
    if ctx.obj.get("json_out", False):
        out(to_json({"errors": errors, "warnings": warnings}))
    else:
        for e in errors:
            out(f"error: {e}")
        for w in warnings:
            out(f"warning: {w}")
        if not errors:
            count = result["count"]
            msg = f"tasks.json is fine ({count} tasks"
            if warnings:
                msg += f", {len(warnings)} warnings"
            msg += ")"
            out(msg)
    if errors:
        raise AtmExit(1)


@app.command("service", rich_help_panel=MAINTENANCE)
def service(
    ctx: typer.Context,
    action: str = typer.Argument("start", click_type=click.Choice(["start", "stop", "status"])),
    open_: bool = typer.Option(False, "--open", help="Also open the dashboard in the browser"),
    all_projects: bool = typer.Option(False, "--all", help="Show all projects overview"),
    port: int = typer.Option(DEFAULT_PORT, "--port", help="The port to try first"),
    dir_: Optional[str] = typer.Option(None, "--dir", help="The project to show"),
    refresh: bool = typer.Option(False, "--refresh", help="Only restart if dashboard is older"),
) -> None:
    """Run the dashboard server in the background."""
    cwd = os.path.abspath(dir_) if dir_ else os.getcwd()
    if not os.path.isdir(cwd):
        raise AtmFailure(f"--dir {dir_}: not a directory", 2)
    home = atm_home()
    dry_run = ctx.obj.get("dry_run", False)
    if refresh:
        if dry_run:
            out("[dry-run] would refresh the dashboard server if outdated")
        else:
            serve_refresh(home)
    elif action == "start":
        if dry_run:
            out("[dry-run] would start the dashboard server")
        else:
            refresh_server(home)
            url = serve_start(cwd, port, all_projects)
            if open_:
                open_browser(url)
    elif action == "stop":
        if dry_run:
            out("[dry-run] would stop the dashboard server")
        else:
            serve_stop(home, port)
    else:
        serve_status(home, ctx.obj.get("json_out", False), port)


@app.command("version", rich_help_panel=META)
def version(ctx: typer.Context) -> None:
    """Print the plugin version."""
    out(to_json({"version": plugin_version()}) if ctx.obj.get("json_out", False) else plugin_version())


@app.command("help", rich_help_panel=META)
def help_(ctx: typer.Context) -> None:
    """Show the list of commands."""
    usage = ctx.parent.get_help()
    out(to_json({"usage": usage}) if ctx.obj.get("json_out", False) else usage)


# ---------------------------------------------------------------------------
# shell completion (T-423)
# ---------------------------------------------------------------------------
# Typer installs the scripts (--install-completion, --show-completion; zsh, bash, fish); these complete atm's own
# values, read through atm-lib in the folder the shell is in. Completion never fails loudly: outside a project, or
# with a task file atm can't read, it completes nothing.

# The targets of `atm task state` (task_state's cmd_map), with what each runs
STATE_TARGETS = [
    ("in_progress", "start"),
    ("in_review", "review"),
    ("done", "done"),
    ("todo", "back to to do: fail, release, reject, unblock or reopen"),
    ("blocked", "block"),
    ("skipped", "skip"),
]


def _vocabulary():
    return backlog_tasks(open_project(os.getcwd()))["vocab"]


def _task_ids():
    return [(v["task"]["id"], v["task"].get("name") or "") for v in task_views(open_project(os.getcwd()), Filters())]


def _sprints():
    return [(s["id"], s.get("goal") or s.get("state") or "") for s in _vocabulary()["sprints"] if s.get("state") != "closed"]


def _releases():
    return [(r["id"], r.get("name") or r.get("state") or "") for r in _vocabulary()["releases"]]


def _components():
    return [(c["id"], c.get("label") or "") for c in _vocabulary()["components"]]


def _completion_terms(key):
    return lambda: [(t["id"], t.get("label") or "") for t in _vocabulary()[key]]


def _completer(values):
    """A Click completer for `values()` ((value, help) pairs), matching what has been typed; nothing on any error."""

    def complete(ctx, param, incomplete):
        from click.shell_completion import CompletionItem

        try:
            pairs = values()
        except Exception:  # completion must never break the shell, whatever the project holds
            return []
        return [CompletionItem(value, help=help or None) for value, help in pairs if value.startswith(incomplete)]

    return complete


# (command path, parameter name) -> completer; a None path is any command with that parameter
COMPLETIONS = {
    (None, "task_id"): _task_ids,
    (None, "task_ids"): _task_ids,
    (None, "ids"): _task_ids,
    (None, "sprint"): _sprints,
    (None, "release"): _releases,
    (None, "component"): _components,
    (None, "status"): _completion_terms("statuses"),
    (None, "type_"): _completion_terms("types"),
    (None, "priority"): _completion_terms("priorities"),
    ("task state", "next_state"): lambda: STATE_TARGETS,
    ("db unarchive", "ids"): None,  # archived tasks, which the task list doesn't hold
    **{(f"sprint {c}", "name"): _sprints for c in ("edit", "start", "close", "delete")},
    ("sprint close", "carry_to"): _sprints,
    ("sprint delete", "move_to"): _sprints,
    ("sprint move", "to"): _sprints,
    **{(f"release {c}", "release_id"): _releases for c in ("edit", "archive", "notes")},
    ("component edit", "component_id"): _components,
}


def _attach(cmd, path):
    for param in getattr(cmd, "params", []):
        key = (path, param.name) if (path, param.name) in COMPLETIONS else (None, param.name)
        values = COMPLETIONS.get(key)
        if values is not None:
            param._custom_shell_complete = _completer(values)
    for name, sub in getattr(cmd, "commands", {}).items():
        _attach(sub, f"{path} {name}".strip())


@app.command("_lock-test", hidden=True)
def lock_test(seconds: float = typer.Argument(0.0)) -> None:
    """Test hook: hold the task folder's lock for `seconds`, so tests can check exclusion."""
    hold_lock_for_test(os.getcwd(), seconds)


def command():
    """atm's Click command, with atm's values to complete."""
    cmd = typer.main.get_command(app)
    _attach(cmd, "")
    return cmd


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

# The options of the whole tree, which may go anywhere on the line: moved before the command, where Click reads them
GLOBAL_FLAGS = ("--json", "-j", "--dry-run", "-n")

# Commands that don't check for an outdated dashboard server first: service does it itself, git runs the merge
# driver in the middle of a merge (the server process isn't a CLI command: atm.py --server, plugin/scripts/atm-serve)
NO_REFRESH = (("service",), ("db", "merge-driver"), ("help",), ("version",), ("_lock-test",))


def report(error, as_json):
    """An error as atm reports it: `atm: <message>` on stderr, or {"error", "code"} with --json."""
    if as_json:
        sys.stderr.write(to_json({"error": str(error), "code": error.code}) + "\n")
    else:
        sys.stderr.write(f"atm: {error}\n")


def leading_dir(argv):
    """`atm --dir PATH <command> ...` (or `--dir=PATH`, T-351): the folder to run in, and the rest. Only before the
    command: `atm service --dir PATH` keeps its own meaning."""
    if argv and argv[0] == "--dir":
        return (argv[1] if len(argv) > 1 else ""), argv[2:]
    if argv and argv[0].startswith("--dir="):
        return argv[0][len("--dir=") :], argv[1:]
    return None, argv


def global_first(argv):
    """argv with the tree's own options (GLOBAL_FLAGS) first, wherever they were given; what follows `--` (the command
    `loop run` runs) is left as it is."""
    split = argv.index("--") if "--" in argv else len(argv)
    head, tail = argv[:split], argv[split:]
    flags = [a for a in head if a in GLOBAL_FLAGS]
    return flags + [a for a in head if a not in GLOBAL_FLAGS] + tail


def command_path(argv):
    """The command's names (`("task", "state")`), up to its first option or argument."""
    names = [a for a in argv if not a.startswith("-")]
    return tuple(names[:2])


def main(argv):
    target, argv = leading_dir(argv)
    as_json = any(a in ("--json", "-j") for a in argv[: argv.index("--") if "--" in argv else len(argv)])
    if target is not None:
        # As if started in that folder: the project is found from there, and relative paths count from there
        if not target or not os.path.isdir(target):
            report(AtmError(f"--dir: no folder '{target}'", 2), as_json)
            return 2
        os.chdir(target)
    argv = global_first(argv)
    path = command_path(argv)
    try:
        if path and not any(path[: len(skip)] == skip for skip in NO_REFRESH):
            try:
                refresh_server()  # after a plugin update, the first atm command brings the dashboard server along
            except Exception:
                pass  # never in the way of the command itself
        code = command().main(args=argv, prog_name="atm", standalone_mode=False)
        return code if isinstance(code, int) else 0
    except click.UsageError as e:
        usage = e.ctx.get_usage() if e.ctx else ""
        report(AtmError(e.format_message() + (f"\n{usage}" if usage and not as_json else ""), 2), as_json)
        return 2
    except click.exceptions.Exit as e:
        return e.exit_code
    except click.Abort:
        return 1
    except AtmFailure as e:
        # atm-lib's errors and those of the modules from before the layers (AtmError is one of them)
        report(e, as_json)
        return e.code
    except AtmExit as e:
        return e.code
    except Exception as e:  # noqa: BLE001
        # A file atm didn't expect (a hostile or damaged one) is an error message, not a traceback: the skills and
        # the merge driver read stderr (docs/security-review.md F12)
        report(
            AtmError(f"unexpected error ({type(e).__name__}: {e}); run atm doctor, and report it if that is clean"),
            as_json,
        )
        return 1


# ==== api/server.py ====
# The dashboard server (the REST API): one per user, a detached process serving every registered project.
#
# The server holds the plugin's dashboard in memory and answers its requests through atm-lib: the projects, their
# tasks (v1), claims, worktrees and logos. It reads no task file itself (docs/layered-architecture.md §6); its routes
# and record are described in docs/multi-project-api.md. It runs as its own process, entered with `serve_main` (the
# built atm.py with `--server`, as plugin/scripts/atm-serve runs it); the CLI starts, stops and refreshes it
# (`cli/serve.py`) and never imports this module.

PORT_TRIES = 100
# Computed from the project's data, under the paths the dashboard has always used (docs/multi-project-api.md)
PROJECT_FILES = ("claims.json", "worktrees.json")
# Task data the server will read into memory and send (a big project's archive is a few MB)
MAX_SERVED_BYTES = 100_000_000
PROJECT_FILE = re.compile(r"/p/([^/]+)/(" + "|".join(re.escape(f) for f in PROJECT_FILES + ("logo",)) + r")")
# A logo is the project's own file, which could be an SVG with a script: it is shown as an image, never run
LOGO_HEADERS = {
    "Content-Security-Policy": "default-src 'none'; style-src 'unsafe-inline'; sandbox",
    "X-Content-Type-Options": "nosniff",
}
# The plugin's own files the dashboard loads beside it (docs/compmd-rendering-design.md §2, §3c): read-only, only
# regular files under plugin/assets/ with these extensions, never through a symlink. CompMD's sandbox frames have an
# opaque origin, so what they load comes cross-origin: every asset may be read from anywhere (they are the plugin's
# published files), and a frame document is sandboxed even when opened alone, as @compmd/vite serves them
ASSETS = "/assets/"
COMPMD_ASSETS = ASSETS + "compmd/"
ASSET_TYPES = {
    ".js": "text/javascript; charset=utf-8",
    ".wasm": "application/wasm",
    ".html": "text/html; charset=utf-8",
}
ASSET_HEADERS = {"Cache-Control": "no-cache", "Access-Control-Allow-Origin": "*"}
FRAME_DOCUMENT_HEADERS = {**ASSET_HEADERS, "Content-Security-Policy": "sandbox allow-scripts"}
ASSET_SEGMENT = re.compile(r"[A-Za-z0-9_@][A-Za-z0-9._@-]*")
# On every response: no content sniffing, and no other site may embed it (docs/security-review.md F11)
SECURITY_HEADERS = {"X-Content-Type-Options": "nosniff", "Cross-Origin-Resource-Policy": "same-origin"}
INLINE_SCRIPT = re.compile(rb"<script\b([^>]*)>(.*?)</script>", re.S | re.I)


def page_csp(page, origin):
    """The dashboard's Content-Security-Policy: only its own inline scripts run (by hash), and the CompMD runtime from
    this server's asset folder, which may compile WebAssembly (docs/compmd-rendering-design.md §2); its only frames are
    CompMD's sandbox frames from that folder (§3c), it talks only to this server, and no page may frame it. Any script
    injected into it, from anywhere else, or a `javascript:` link, is refused by the browser. `origin` is the page's
    (`http://localhost:<port>`, from the checked Host header): a source with a path can't be written without it.
    tests/e2e/csp.mjs builds the same policy, so the e2e suite runs under it."""
    hashes = [
        "'sha256-" + base64.b64encode(hashlib.sha256(body).digest()).decode("ascii") + "'"
        for attrs, body in INLINE_SCRIPT.findall(page)
        if b"src=" not in attrs
    ]
    return "; ".join(
        [
            "default-src 'none'",
            "script-src " + " ".join(hashes + ["'wasm-unsafe-eval'", origin + COMPMD_ASSETS]),
            "frame-src " + origin + COMPMD_ASSETS,
            "style-src 'unsafe-inline'",
            "img-src 'self' data:",
            "connect-src 'self'",
            "base-uri 'none'",
            "form-action 'none'",
            "frame-ancestors 'none'",
        ]
    )


def port_in_use(port):
    """Something accepts connections on 127.0.0.1:port or [::1]:port (`localhost` may resolve to either)."""
    for family, host in ((socket.AF_INET, "127.0.0.1"), (socket.AF_INET6, "::1")):
        try:
            with socket.socket(family, socket.SOCK_STREAM) as s:
                s.settimeout(0.5)
                if s.connect_ex((host, port)) == 0:
                    return True
        except OSError:  # no IPv6 on this machine
            pass
    return False


def asset_file(path, root=None):
    """(absolute path, content type) of the plugin file a `/assets/…` request names, or None: each segment a plain
    name (no `.` or `..`, no hidden file, nothing percent-encoded), a known extension, a regular file under
    plugin/assets/ reached through no symlink."""
    if not path.startswith(ASSETS):
        return None
    segments = path[len(ASSETS) :].split("/")
    if not all(ASSET_SEGMENT.fullmatch(s) for s in segments):
        return None
    content_type = ASSET_TYPES.get(os.path.splitext(segments[-1])[1])
    if content_type is None:
        return None
    current = root or os.path.join(PLUGIN_ROOT, "assets")
    if os.path.islink(current):
        return None
    for segment in segments:
        current = os.path.join(current, segment)
        if os.path.islink(current):
            return None
    return (current, content_type) if os.path.isfile(current) else None


def logo_stamp(p):
    """(size, nanosecond mtime) of the logo's file, or None if it doesn't exist."""
    try:
        st = os.stat(p)
    except OSError:
        return None
    return (st.st_size, st.st_mtime_ns)


def file_etag(stamp):
    """A strong ETag from a file's size and nanosecond mtime. Last-Modified alone has one-second resolution, so two
    atm changes in the same second would look unchanged to the dashboard."""
    return f'"{stamp[0]:x}-{stamp[1]:x}"'


class ServedProjects:
    """The registry as the server sees it, re-read when it changes."""

    def __init__(self, home):
        self.registry = open_registry(home)
        self.stamp = None
        self.projects = []

    def entries(self):
        stamp = self.registry.stamp()
        if stamp != self.stamp:
            self.projects = self.registry.entries()
            self.stamp = stamp
        return self.projects

    def find(self, project):
        return next((e for e in self.entries() if e.get("id") == project), None)


class Summaries:
    """Each registered project, opened through boot, and its summary (atm-lib's project_summary), computed again only
    when its task data or claims changed, or the day did (the completions count by day)."""

    def __init__(self):
        self.openers = {}  # project folder -> boot's opener for it (it finds where the data is once: that runs git)
        self.cache = {}  # project folder -> (stamp, summary or None)

    def project(self, entry, tolerant=False):
        """The project, opened afresh for each request: atm-lib reads one snapshot of its data per call. `tolerant`
        reads a malformed task file as far as it can be read, for showing it (T-377). A task folder atm refuses (a
        symlink, docs/security-review.md F7, F8) raises AtmFailure."""
        if entry["path"] not in self.openers:
            self.openers[entry["path"]] = project_opener(entry["path"])
        return self.openers[entry["path"]](tolerant=tolerant)

    def data_stamp(self, entry):
        """The project's store stamp ({tasks, archive, claims, head}), or None when the project is missing or refused."""
        try:
            stamp = project_stamp(self.project(entry))
        except AtmFailure:
            return None
        return stamp if stamp["tasks"] is not None else None

    def stamp(self, entry):
        """What the summary depends on, or None when the project is missing: its task data, claims, the day, and the
        checked-out branch (on the card)."""
        stamp = self.data_stamp(entry)
        if stamp is None:
            return None
        return (stamp["tasks"], stamp["claims"], now().date().isoformat(), stamp["head"])

    def summary(self, entry, stamp):
        cached = self.cache.get(entry["path"])
        if cached and cached[0] == stamp:
            return cached[1]
        try:
            summary = project_summary(self.project(entry))
        except AtmFailure:
            summary = None  # unreadable tasks.json or an unusable vocabulary: listed as missing
        self.cache[entry["path"]] = (stamp, summary)
        return summary


def project_logo(summaries, entry):
    """(absolute path, content type) of a registered project's logo (meta.logo, or one found), or None."""
    logo = get_logo(summaries.project(entry, tolerant=True))["found"]
    return (logo["file"], logo["type"]) if logo else None


def quietly(fn, default):
    """fn(), or `default` if it fails: one project's broken files never break the list of all projects (F18)."""
    try:
        return fn()
    except Exception:  # noqa: BLE001 - any failure in one project's files
        return default


def api_projects(registry, summaries):
    """(body, ETag) of /api/projects. The ETag changes with the registry, and with each project's tasks.json, claims,
    logo and the day. A project whose files can't be read is listed as missing; the others are unaffected."""
    entries = registry.entries()
    stamps = [quietly(lambda e=e: summaries.stamp(e), None) for e in entries]
    trees = [
        quietly(lambda e=e: working_copies(summaries.project(e)), []) if stamp else []
        for e, stamp in zip(entries, stamps)
    ]
    logos = [quietly(lambda e=e: project_logo(summaries, e), None) if stamp else None for e, stamp in zip(entries, stamps)]
    logo_stamps = [logo_stamp(logo[0]) if logo else None for logo in logos]
    etag = (
        '"'
        + hashlib.sha1(repr([registry.stamp] + stamps + trees + logo_stamps).encode("utf-8")).hexdigest()[:16]
        + '"'
    )

    def body():
        projects = []
        for entry, stamp, tree, logo in zip(entries, stamps, trees, logos):
            summary = quietly(lambda: summaries.summary(entry, stamp), None) if stamp else None
            projects.append(
                {
                    "id": entry.get("id"),
                    "name": entry.get("name"),
                    "path": entry["path"],
                    "missing": summary is None,
                    "summary": summary,
                    "worktrees": tree if summary else [],
                    # Relative to the dashboard, like the project's files; a new one shows as a new ETag
                    "logo": f"p/{entry.get('id')}/logo" if logo and summary else None,
                }
            )
        return to_json({"generated_at": now_iso(), "projects": projects}).encode("utf-8")

    return body, etag


SEARCH_QUERY_CAP = 200  # characters of the query that count
SEARCH_RESULTS_CAP = 200  # task IDs per project


def api_search(registry, summaries, query):
    """The body of /api/search?q=: per project, the IDs of the tasks.json tasks (not the archive) in which every word of
    the query appears, case-insensitively, as the dashboard's own search finds them (atm-lib's search_tasks); the
    project's name counts as part of every task's text. Read-only, like every route; `{}` for an empty query."""
    query = query[:SEARCH_QUERY_CAP]
    results = {}
    if not query.split():
        return to_json(results).encode("utf-8")
    for entry in registry.entries():

        def matches(entry=entry):
            stamp = summaries.data_stamp(entry)
            if stamp is None or stamp["tasks"][0] > MAX_SERVED_BYTES:
                return []
            return search_tasks(summaries.project(entry), query, also=str(entry.get("name") or ""))

        ids = quietly(matches, [])
        if ids:
            results[entry.get("id")] = ids[:SEARCH_RESULTS_CAP]
    return to_json(results).encode("utf-8")


def stamp_etag(*parts):
    """A strong ETag from what a response depends on (store stamps, the query)."""
    return '"' + hashlib.sha1(repr(list(parts)).encode("utf-8")).hexdigest()[:16] + '"'


def api_claims(summaries, entry, stamp):
    """(body, ETag) of a project's claims.json: the claims of all its worktrees (atm-lib's list_claims, from the shared
    claims store; each worktree's own snapshot has only the claims made there); none before the first claim."""

    def body():
        snapshot = {"generated_at": now_iso(), "claims": list_claims(summaries.project(entry))}
        return to_json(snapshot).encode("utf-8")

    return body, stamp_etag("claims", stamp["claims"])


V1_TASKS = re.compile(r"/api/v1/projects/([^/]+)/tasks")
# A project's files for the document viewer (docs/compmd-viewer-design.md §4): `documents` as JSON (what the file is,
# and the CompMD document to render), `files` an image's bytes for an <img>
V1_FILES = re.compile(r"/api/v1/projects/([^/]+)/(documents|files)")
# atm-lib's refusals, as HTTP statuses: a malformed path, no such file, one atm doesn't show, one too large
FILE_ERRORS = ((Invalid, 400), (NotFound, 404), (TooLarge, 413), (Refused, 403))
LIST_PARAMETERS = ("status", "label", "sprint")  # repeatable: ?status=todo&status=blocked
FLAG_PARAMETERS = ("backlog", "ready", "blocked", "in_pr", "with_user", "archived")
VALUE_PARAMETERS = ("type", "priority", "agent", "release", "component")


def task_filters(query):
    """atm-lib's Filters from a v1 route's query string (docs/layered-architecture.md §6.2): the list filters
    repeatable, the flags `true` or `1`, other parameters ignored."""
    given = parse_qs(query)
    filters = {name: given[name] for name in LIST_PARAMETERS if name in given}
    filters.update({name: given[name][0] for name in VALUE_PARAMETERS if name in given})
    filters.update({name: given[name][0].lower() in ("true", "1") for name in FLAG_PARAMETERS if name in given})
    return Filters(**filters)


def api_v1_tasks(summaries, entry, query, stamp):
    """(body, ETag) of GET /api/v1/projects/<p>/tasks: the project's tasks the filters keep, each with its derived
    values (atm-lib's task_views), and in the same snapshot the project's meta and how big its task data is. The ETag
    changes with the task data, the archive and the query."""
    etag = stamp_etag(stamp["tasks"], stamp["archive"], query)

    def body():
        # Read tolerantly: the dashboard shows a malformed task file, with what is wrong with it (T-377)
        snapshot = task_snapshot(summaries.project(entry, tolerant=True), task_filters(query))
        return to_json({"generated_at": now_iso(), **snapshot}).encode("utf-8")

    return body, etag


def api_worktrees(summaries, entry, stamp):
    """(body, ETag) of a project's worktrees.json: its working copies, main first, and for each other one the tasks
    whose copy there differs from the main one's (atm-lib's worktree_copies). The dashboard shows them all on one
    board, each task from the working copy where it got furthest. The ETag changes with the copies and the claims
    (a decision made in another worktree hides a stale copy)."""
    def body():
        copies = worktree_copies(summaries.project(entry))
        return to_json({"generated_at": now_iso(), "worktrees": copies}).encode("utf-8")

    return body, stamp_etag(stamp["copies"], stamp["claims"])


def serve_main(argv):
    """The server process's entry (`atm.py --server <home> <port>`): its arguments, then `serve`. Returns its exit
    code: 2 for arguments it can't use."""
    if len(argv) != 2 or not argv[1].isdigit():
        sys.stderr.write("atm-serve: usage: atm-serve <atm folder> <first port>\n")
        return 2
    serve(os.path.abspath(argv[0]), int(argv[1]))
    return 0


def serve(home, port):
    """The detached server process: serve on the first free port from `port`, record it in `home`, run until
    stopped."""
    import http.server
    import socketserver

    record_path = os.path.join(home, SERVER_RECORD)
    with open(TEMPLATE, "rb") as f:
        page = f.read()
    # From the bytes it serves, not a second read of the file: a build writing the file in between would make whoami
    # report another build than the page has, and the page would offer a reload forever
    version, build = page_tags(page[:HEAD_BYTES].decode("utf-8", errors="replace"))
    # The policy names the page's own origin, so there is one per Host the server accepts (localhost, 127.0.0.1)
    page_policies = {}
    # Which atm folder this server serves, as an ID (common/dashboard.py home_id): `atm serve stop` finds it by this
    # even if its record was lost (T-390)
    served_home = home_id(home)
    registry = ServedProjects(home)
    summaries = Summaries()

    class Handler(http.server.BaseHTTPRequestHandler):
        server_version = "atm"
        # A connection that sends nothing (or trickles) is dropped instead of holding a thread forever (F18)
        timeout = 30

        def send(self, status, body=b"", content_type="application/json", etag=None, headers=None):
            self.send_response(status)
            if body or status == 200:
                self.send_header("Content-Type", content_type)
                self.send_header("Content-Length", str(len(body)))
            if etag:
                self.send_header("ETag", etag)
            for name, value in {"Cache-Control": "no-store", **SECURITY_HEADERS, **(headers or {})}.items():
                self.send_header(name, value)
            self.end_headers()
            if body and self.command != "HEAD":
                self.wfile.write(body)

        def send_error_json(self, status, message):
            self.send(status, to_json({"error": message}).encode("utf-8"))

        def send_conditional(self, body_fn, etag, content_type="application/json", headers=None):
            if self.headers.get("If-None-Match") == etag:
                # Unchanged since the dashboard's last poll: it skips parsing and re-rendering (D-5.1)
                self.send(304, etag=etag, headers=headers)
                return
            self.send(200, body_fn(), content_type, etag, headers)

        def do_GET(self):
            # One bad request, or one project's broken files, gets a 500, never a dead handler thread with no answer
            try:
                self.handle_get()
            except Exception:  # noqa: BLE001
                try:
                    self.send_error_json(500, "internal error")
                except OSError:
                    pass

        def handle_get(self):
            port_now = self.server.server_address[1]
            if self.headers.get("Host") not in (f"localhost:{port_now}", f"127.0.0.1:{port_now}"):
                # A page on another site that points its own domain at 127.0.0.1 (DNS rebinding) is refused
                self.send_error_json(403, "unexpected Host header")
                return
            path = urlsplit(self.path).path
            if path == "/":
                host = self.headers.get("Host")
                if host not in page_policies:
                    page_policies[host] = page_csp(page, "http://" + host)
                policy = {"Content-Security-Policy": page_policies[host]}
                self.send(200, page, "text/html; charset=utf-8", headers=policy)
            elif path.startswith(ASSETS):
                self.serve_asset(path)
            elif path == WHOAMI:
                info = {
                    "atm": True,
                    "pid": os.getpid(),
                    "port": port_now,
                    "version": version,
                    "build": build,
                    "home": served_home,
                }
                self.send(200, to_json(info).encode("utf-8"))
            elif path == "/api/projects":
                body, etag = api_projects(registry, summaries)
                self.send_conditional(body, etag)
            elif path == "/api/search":
                query = parse_qs(urlsplit(self.path).query).get("q", [""])[0]
                self.send(200, api_search(registry, summaries, query))
            elif V1_TASKS.fullmatch(path):
                self.serve_v1_tasks(V1_TASKS.fullmatch(path).group(1), urlsplit(self.path).query)
            elif V1_FILES.fullmatch(path):
                match = V1_FILES.fullmatch(path)
                self.serve_v1_file(match.group(1), match.group(2), urlsplit(self.path).query)
            else:
                self.serve_project_file(path)

        def serve_v1_tasks(self, project, query):
            entry = registry.find(project)
            if entry is None:
                self.send_error_json(404, "unknown project")
                return
            stamp = summaries.data_stamp(entry)
            if stamp is None:
                self.send_error_json(404, "not found")
                return
            if stamp["tasks"][0] > MAX_SERVED_BYTES:
                # Each request reads the whole task data into memory: a runaway file is refused, not served
                self.send_error_json(413, f"larger than {MAX_SERVED_BYTES // 1_000_000} MB")
                return
            body, etag = api_v1_tasks(summaries, entry, query, stamp)
            if self.headers.get("If-None-Match") == etag:
                self.send(304, etag=etag)
                return
            try:
                content = body()
            except AtmFailure as e:
                # A filter naming no such sprint or release (400), an unreadable task file (500)
                self.send_error_json(400 if e.code == 2 else 500, str(e))
                return
            self.send(200, content, "application/json", etag)

        do_HEAD = do_GET

        def serve_v1_file(self, project, route, query):
            """GET /api/v1/projects/<p>/documents?path= (the viewer's JSON) or …/files?path= (an image), through
            atm-lib, which checks the path and reads the file from the working copy (lib/documents.py). The ETag is
            the file's size and mtime (and, for a document, the current commit, which it names), so the viewer's poll
            gets a 304 without the file being read again."""
            entry = registry.find(project)
            if entry is None:
                self.send_error_json(404, "unknown project")
                return
            if summaries.data_stamp(entry) is None:
                self.send_error_json(404, "not found")
                return
            path = parse_qs(query).get("path", [""])[0]
            opened = summaries.project(entry, tolerant=True)
            try:
                stamp = document_stamp(opened, path)
                if route == "documents":
                    etag = stamp_etag("document", stamp, opened.vcs.head())
                    if self.headers.get("If-None-Match") == etag:
                        self.send(304, etag=etag)
                        return
                    self.send(200, to_json(read_document(opened, path)).encode("utf-8"), "application/json", etag)
                    return
                etag = file_etag(stamp)
                headers = {**LOGO_HEADERS, "Cache-Control": "no-cache"}
                if self.headers.get("If-None-Match") == etag:
                    self.send(304, etag=etag, headers=headers)
                    return
                data, content_type = read_image(opened, path)
                self.send(200, data, content_type, etag, headers)
            except AtmFailure as e:
                status = next((code for kind, code in FILE_ERRORS if isinstance(e, kind)), 500)
                self.send_error_json(status, str(e))

        def serve_project_file(self, path):
            match = PROJECT_FILE.fullmatch(path)
            if not match:
                self.send_error_json(404, "not found")
                return
            entry = registry.find(match.group(1))
            if entry is None:
                self.send_error_json(404, "unknown project")
                return
            name = match.group(2)
            stamp = summaries.data_stamp(entry)
            if stamp is None:
                self.send_error_json(404, "not found")
                return
            if name == "logo":
                self.serve_logo(entry)
            elif name == "worktrees.json":
                self.send_conditional(*api_worktrees(summaries, entry, stamp))
            else:
                self.send_conditional(*api_claims(summaries, entry, stamp))

        def serve_logo(self, entry):
            """The project's logo (the store checks it is inside the project and small enough). The browser may keep
            it, asking again each time (no-cache) and getting a 304 while it is unchanged."""
            logo = project_logo(summaries, entry)
            stamp = logo_stamp(logo[0]) if logo else None
            if stamp is None:
                self.send_error_json(404, "no logo")
                return

            def read():
                with open(logo[0], "rb") as f:
                    return f.read()

            self.send_conditional(read, file_etag(stamp), logo[1], {**LOGO_HEADERS, "Cache-Control": "no-cache"})

        def serve_asset(self, path):
            """A plugin file the dashboard loads (the CompMD runtime): asked again each time (no-cache), a 304 while
            it is unchanged. Its folder is named for the version, so a page never gets another version's file."""
            found = asset_file(path)
            stamp = logo_stamp(found[0]) if found else None
            if stamp is None:
                self.send_error_json(404, "not found")
                return

            def read():
                with open(found[0], "rb") as f:
                    return f.read()

            headers = FRAME_DOCUMENT_HEADERS if found[0].endswith(".html") else ASSET_HEADERS
            self.send_conditional(read, file_etag(stamp), found[1], headers)

        def not_allowed(self):
            self.send_error_json(405, "only GET and HEAD")

        do_POST = do_PUT = do_DELETE = do_PATCH = do_OPTIONS = not_allowed

    class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
        daemon_threads = True
        # A restarted server takes its port back while the old one's connections linger in TIME_WAIT. On macOS the
        # same option would also let it share a port another server listens on, hence port_in_use() first.
        allow_reuse_address = True

    class Server6(Server):
        address_family = socket.AF_INET6

    # Browsers, curl and urllib try [::1] before 127.0.0.1 for `localhost`: the server listens on both, so no other
    # local user can take [::1] on our port and answer for us (docs/security-review.md F10). Without IPv6, 127.0.0.1
    # alone is enough: nobody can listen on [::1] then.
    server = server6 = None
    for candidate in range(port, port + PORT_TRIES):
        if port_in_use(candidate):
            continue
        try:
            server = Server(("127.0.0.1", candidate), Handler)
        except OSError:
            continue
        try:
            server6 = Server6(("::1", candidate), Handler)
        except OSError as e:
            if e.errno == errno.EADDRINUSE:
                server.server_close()
                server = None
                continue
        break
    if server is None:
        sys.stderr.write(f"atm: no free port in {port}-{port + PORT_TRIES - 1}\n")
        return
    if server6 is not None:
        threading.Thread(target=server6.serve_forever, daemon=True).start()
    record = {
        "pid": os.getpid(),
        "port": server.server_address[1],
        "version": version,
        "build": build,
        "plugin_root": PLUGIN_ROOT,
        "home": served_home,
        "started_at": now_iso(),
    }
    write_json_atomic(record_path, record)

    def stop(signum, frame):
        if (read_json(record_path) or {}).get("pid") == os.getpid():
            os.remove(record_path)
        os._exit(0)

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    server.serve_forever()


if __name__ == "__main__":
    if sys.argv[1:2] == [SERVER_ENTRY]:
        sys.exit(serve_main(sys.argv[2:]))
    sys.exit(main(sys.argv[1:]))
