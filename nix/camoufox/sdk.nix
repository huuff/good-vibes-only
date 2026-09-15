{
  lib,
  python3Packages,
  fetchPypi,
}:
python3Packages.buildPythonPackage {
  pname = "camoufox";
  version = "0.5.6";
  pyproject = true;
  src = fetchPypi {
    pname = "camoufox";
    version = "0.5.6";
    hash = "sha256-5K5z7JEzABJo7wdlU0/aDlRVCBKhmFDxQXVs1XL+k7k=";
  };
  # No semaphore/shared-memory access is needed when all addons are excluded.
  patches = [ ./no-empty-addon-lock.patch ];
  build-system = [ python3Packages.poetry-core ];
  dependencies = with python3Packages; [
    browserforge
    inquirer
    language-tags
    lxml
    numpy
    orjson
    platformdirs
    playwright
    pysocks
    pyyaml
    requests
    rich
    rich-click
    screeninfo
    typing-extensions
    ua-parser
  ];
  pythonImportsCheck = [ "camoufox.utils" ];
  doCheck = false;
  meta = {
    description = "Pinned Camoufox launch-settings SDK";
    homepage = "https://github.com/daijro/camoufox";
    license = lib.licenses.mit;
  };
}
