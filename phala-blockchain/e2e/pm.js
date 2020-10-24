const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

function once(fn) {
	let fired = false;
	return (...args) => {
		if (!fired) {
			fn(...args);
			fired = true;
		}
	};
}

class Process {
	constructor(..._args) {
		this.args = _args;
		this.process = null;
		this.promiseStopped = null;
		this.exitCode = null;
		this.errorCode = null;
		this.stopped = false;
	}
	start() {
		const process = spawn(...this.args);
		this.process = process;
		this._listenEvents();
	}
	async startAndWaitForOutput(pattern) {
		const process = spawn(...this.args);
		this.process = process;
		await new Promise((resolve, reject) => {
			process.stdout.on('data', (data) => {
				if (pattern.test(data)) {
					resolve();
				}
			});
			process.stderr.on('data', (data) => {
				if (pattern.test(data)) {
					resolve();
				}
			})
			this._listenEvents(reject);
		});
	}
	_listenEvents(fallbackReject) {
		this.promiStopped = new Promise((resolve, _reject) => {
			const handle = once((code) => {
				this.stopped = true;
				this.exitCode = code;
				if (fallbackReject) {
					fallbackReject(new Error('Got an error with ' + code));
				}
				console.log(`Process ${this.process.pid} exited with code ${code}`);
				resolve(code);
			});
			this.process.on('error', handle);
			this.process.on('close', handle);
			this.process.on('exit', handle);
		});
	}
	async kill(sig) {
		this.process.kill(sig)
		return await this.promiseStopped;
	}
}

class TempDir {
	constructor(prefix='phala-e2e-') {
		this.dir = fs.mkdtempSync(prefix);
	}
	cleanup() {
		rimraf(this.dir);
	}
}

function rimraf(dir_path) {
    if (fs.existsSync(dir_path)) {
        fs.readdirSync(dir_path).forEach(function(entry) {
            var entry_path = path.join(dir_path, entry);
            if (fs.lstatSync(entry_path).isDirectory()) {
                rimraf(entry_path);
            } else {
                fs.unlinkSync(entry_path);
            }
        });
        fs.rmdirSync(dir_path);
    }
}


module.exports = { Process, TempDir };
