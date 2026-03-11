from setuptools import setup

setup(
    name="zendcli",
    version="0.1.0",
    py_modules=["zendcli"],
    install_requires=["click>=8.0", "requests>=2.28"],
    entry_points={
        "console_scripts": [
            "zend=zendcli:cli",
        ],
    },
)
