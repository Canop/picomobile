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

async function initializeCam() {
    let toggle_button = document.getElementById('cam-toggle');
    let img = document.getElementById('cam-img');
    let config_div = document.getElementById('cam-config');
    const widgets = {
        resolution: document.getElementById('cam-resolution'),
        enable: document.getElementById('enable-motion-detection'),
        sound: document.getElementById('play-sound-on-motion'),
        save: document.getElementById('save-motion-events'),
    };
    widgets.enable.addEventListener('change', updateSubChecks);
    let config;
    async function fetchConfig() {
        const res = await fetch('/api/cam-config?v=' + Date.now());
        config = await res.json();
        console.log("cam config:", config);
        widgets.resolution.value = config.resolution;
        widgets.enable.checked = config.enable_motion_detection;
        widgets.sound.checked = config.play_sound_on_motion;
        widgets.save.checked = config.save_motion_events;
        updateSubChecks();
    }
    function updateSubChecks() {
        if (widgets.enable.checked) {
            widgets.sound.removeAttribute('disabled');
            widgets.save.removeAttribute('disabled');
        } else {
            widgets.sound.setAttribute('disabled', 'true');
            widgets.save.setAttribute('disabled', 'true');
        }
    }
    async function toggleCam() {
        if (img.style.display === 'none') {
            await fetchConfig();
            img.style.display = 'block';
            config_div.style.display = 'block';
            img.src = "/api/video";
            toggle_button.textContent = 'Close Camera Stream';
        } else {
            img.style.display = 'none';
            config_div.style.display = 'none';
            img.src = "";
            toggle_button.textContent = 'Open Camera Stream';
        }
    }
    document.getElementById('cam-config-save').addEventListener('click', async () => {
        const status = document.getElementById('cam-config-status');
        const update = {
            resolution: widgets.resolution.value,
            enable_motion_detection: widgets.enable.checked,
            play_sound_on_motion: widgets.sound.checked,
            save_motion_events: widgets.save.checked,
        };
        try {
            const res = await fetch('/api/cam-config', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(update),
            });
            if (res.ok) {
                status.textContent = '✓';
                setTimeout(() => status.textContent = '', 2000);
            } else {
                status.textContent = 'Error saving config';
            }
            updateSubChecks();
            if (update.resolution !== config.resolution) {
                const img = document.getElementById('cam-img');
                img.src = '';
                setTimeout(() => {
                    img.src = '/api/video';
                }, 1000);
            }
        } catch (e) {
            status.textContent = 'network error';
        }
    });
    toggle_button.addEventListener('click', toggleCam);
    await toggleCam(); // starting with the camera closed
}


document.addEventListener('DOMContentLoaded', async () => {
    initializeButtons();
    initializeKeyboard();
    await initializeCam();
    repeatLoop();
});
