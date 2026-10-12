"""Quick Dev's process-local Windows Uvicorn reload compatibility hook."""
import os
import sys


def _uvicorn_arguments():
    original = getattr(sys, "orig_argv", ())
    if "-m" in original:
        index = original.index("-m")
        if original[index + 1:index + 2] == ["uvicorn"]:
            return list(original[index + 2:])
    for argument in original[1:]:
        if os.path.basename(argument).lower() in ("uvicorn.exe", "uvicorn-script.py"):
            return list(sys.argv[1:])
    return None


def _original_sitecustomize():
    import importlib.machinery
    import importlib.util
    ours = os.path.normcase(os.path.realpath(os.path.dirname(__file__)))
    paths = [entry for entry in sys.path
             if os.path.normcase(os.path.realpath(entry or os.getcwd())) != ours]
    spec = importlib.machinery.PathFinder.find_spec("sitecustomize", paths)
    if spec and spec.loader:
        module = importlib.util.module_from_spec(spec)
        sys.modules["sitecustomize"] = module
        spec.loader.exec_module(module)


class _ConsoleOutput:
    def __init__(self, output):
        self.output = output
        self.console = open("CONOUT$", "w")

    def write(self, value):
        result = self.output.write(value)
        # Uvicorn's wake-up write must reach a console, even with piped logs.
        self.console.write(" ")
        self.console.flush()
        return result

    def flush(self):
        self.output.flush()

    def __getattr__(self, name):
        return getattr(self.output, name)


def _start_isolated(arguments):
    import subprocess
    environment = os.environ.copy()
    environment["QUICKDEV_UVICORN_ISOLATED"] = "1"
    environment["PYTHONUNBUFFERED"] = "1"
    environment["PYTHONUTF8"] = "1"
    executable = getattr(sys, "_base_executable", sys.executable)
    if executable != sys.executable:
        environment["__PYVENV_LAUNCHER__"] = sys.executable
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = subprocess.SW_HIDE
    # No BREAKAWAY flag: this process and its workers remain in the same Job.
    child = subprocess.Popen(
        [executable, "-u", "-m", "uvicorn", *arguments],
        creationflags=subprocess.CREATE_NEW_CONSOLE, startupinfo=startup,
        stdin=subprocess.DEVNULL, stdout=sys.stdout, stderr=sys.stderr, env=environment,
    )
    try:
        status = child.wait()
    except KeyboardInterrupt:
        subprocess.run(["taskkill", "/F", "/T", "/PID", str(child.pid)],
                       creationflags=subprocess.CREATE_NO_WINDOW,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        child.wait()
        status = 130
    sys.stdout.flush()
    sys.stderr.flush()
    # Exiting during site initialization via SystemExit causes a fatal import
    # error and loses the server's exit code. The supervising process is done.
    status &= 0xFFFFFFFF
    if status >= 0x80000000:
        status -= 0x100000000
    os._exit(status)


if os.name == "nt" and os.environ.get("QUICKDEV_RELOAD_GUARD") == "1":
    arguments = _uvicorn_arguments()
    reload_requested = arguments is not None and (
        "--reload" in arguments or os.environ.get("UVICORN_RELOAD", "").lower() in ("1", "true"))
    isolated = os.environ.get("QUICKDEV_UVICORN_ISOLATED") == "1"
    if reload_requested and not isolated and os.environ.get("QUICKDEV_DISABLE_RELOAD_GUARD") != "1":
        _start_isolated(arguments)
    _original_sitecustomize()
    if reload_requested and isolated:
        import ctypes
        if not ctypes.windll.kernel32.SetConsoleCtrlHandler(None, False):
            raise OSError("Quick Dev could not enable the isolated reload console")
        sys.stdout = _ConsoleOutput(sys.stdout)
        print("[Quick Dev] Windows Uvicorn 热重载控制台已隔离", file=sys.stderr, flush=True)
else:
    _original_sitecustomize()
