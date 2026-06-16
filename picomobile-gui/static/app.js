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

document.addEventListener('DOMContentLoaded', () => {
    initializeButtons();
    initializeKeyboard();
    repeatLoop();
});
