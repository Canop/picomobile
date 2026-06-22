const pressed = {
    forward: false,
    backward: false,
    left: false,
    right: false,
};

async function sendCommand(command) {
    try {
        const response = await fetch('/api/command', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ command }),
        });

        if (!response.ok) {
            console.error(`Command failed: ${command}`);
        }
    } catch (error) {
        console.error(`Error sending "${command}":`, error);
    }
}

function press(action) {
    if (pressed[action]) {
        return;
    }

    pressed[action] = true;

    // Immediate transmission for low latency.
    sendCommand(action);
}

function release(action) {
    if (!pressed[action]) {
        return;
    }

    pressed[action] = false;

    if (
        (action === 'forward' || action === 'backward') &&
        !pressed.forward &&
        !pressed.backward
    ) {
        sendCommand('stop');
    }

    if (
        (action === 'left' || action === 'right') &&
        !pressed.left &&
        !pressed.right
    ) {
        sendCommand('center');
    }
}

async function repeatLoop() {
    while (true) {
        if (pressed.forward) {
            sendCommand('forward');
        } else if (pressed.backward) {
            sendCommand('backward');
        }

        if (pressed.left) {
            sendCommand('left');
        } else if (pressed.right) {
            sendCommand('right');
        }

        await new Promise(resolve => setTimeout(resolve, 50));
    }
}

function initializeButtons() {
    document.querySelectorAll('[data-action]').forEach(button => {
        const action = button.dataset.action;

        button.addEventListener('mousedown', () => press(action));
        button.addEventListener('mouseup', () => release(action));

        button.addEventListener('mouseleave', () => {
            if (pressed[action]) {
                release(action);
            }
        });

        // Touch support for phones/tablets.
        button.addEventListener('touchstart', event => {
            event.preventDefault();
            press(action);
        });

        button.addEventListener('touchend', event => {
            event.preventDefault();
            release(action);
        });

        button.addEventListener('touchcancel', event => {
            event.preventDefault();
            release(action);
        });
    });
}

function initializeKeyboard() {
    const keyMap = {
        ArrowUp: 'forward',
        ArrowDown: 'backward',
        ArrowLeft: 'left',
        ArrowRight: 'right',
    };

    document.addEventListener('keydown', event => {
        const action = keyMap[event.key];

        if (!action) {
            return;
        }

        event.preventDefault();

        if (!event.repeat) {
            press(action);
        }
    });

    document.addEventListener('keyup', event => {
        const action = keyMap[event.key];

        if (!action) {
            return;
        }

        event.preventDefault();
        release(action);
    });

    // Prevent "stuck" keys if focus is lost.
    window.addEventListener('blur', () => {
        for (const action of Object.keys(pressed)) {
            release(action);
        }
    });
}

function initializeCam() {
    let button = document.getElementById('cam-toggle');
    let img = document.getElementById('cam-img');
    let motion_config = document.getElementById('motion-config');
    function toggleCam() {
        if (img.style.display === 'none') {
            img.style.display = 'block';
            motion_config.style.display = 'block';
            img.src = "/api/video";
            button.textContent = 'Close Camera Stream';
        } else {
            img.style.display = 'none';
            motion_config.style.display = 'none';
            img.src = "";
            button.textContent = 'Open Camera Stream';
        }
    }
    button.addEventListener('click', toggleCam);
    toggleCam();
}

async function initializeMotionConfig() {
    const res = await fetch('/api/motion-config');
    const config = await res.json();
    document.getElementById('enable-motion').checked = config.enable_motion_detection;
    document.getElementById('sound-on-motion').checked = config.sound_on_motion;
    document.getElementById('save-motion-events').checked = config.save_motion_events;

    document.getElementById('motion-config-save').addEventListener('click', async () => {
        const status = document.getElementById('motion-config-status');
        const update = {
            enable_motion_detection: document.getElementById('enable-motion').checked,
            sound_on_motion: document.getElementById('sound-on-motion').checked,
            save_motion_events: document.getElementById('save-motion-events').checked,
        };
        try {
            const res = await fetch('/api/motion-config', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(update),
            });
            if (res.ok) {
                status.textContent = '✓';
                setTimeout(() => status.textContent = '', 2000);
            } else {
                status.textContent = 'Erreur';
            }
        } catch (e) {
            status.textContent = 'Erreur réseau';
        }
    });
}

document.addEventListener('DOMContentLoaded', () => {
    initializeButtons();
    initializeKeyboard();
    initializeCam();
    initializeMotionConfig();
    repeatLoop();
});
