let targets_by_id = {};
async function get_target_status(target) {
    return await fetch("/status/" + target.id)
        .then(response => response.json());
}

function create_target_dom(target, status) {
    let div = document.createElement("div");
    update_target_dom(div, target, status);
    return div;
}

function update_target_dom(div, target, status) {
    div.className = "target";
    if (div.children.length == 0) {
        let title = document.createElement("h2");
        title.innerHTML = target.name;
        let description = document.createElement("span");
        let status_line = document.createElement("div");
        status_line.className = "status_line";
        let horizontal_layout = document.createElement("div");
        horizontal_layout.className = "horizontal";
        horizontal_layout.appendChild(title);
        horizontal_layout.appendChild(description);
        div.appendChild(horizontal_layout);
        div.appendChild(status_line);
    }

    let tooltip = "";
    let color = "";
    if (status === null) {
        tooltip = "No data";
        color = "#ffc107";
    } else {
        let timestamp = moment.utc(status.timestamp);
        let formatted_timestamp = timestamp.format("YYYY-MM-DD HH:mm");
        tooltip = "Last checked: " + formatted_timestamp;
        color = {
            "Healthy": "#5cdd8b",
            "Unhealthy": "#dc3545",
            "Degraded": "#ffc107"
        } [status.status];
    }
    div.children[0].children[0].innerHTML = target.name;
    div.children[0].children[1].innerHTML = status.description;
    div.children[1].style.backgroundColor = color;
    div.children[1].setAttribute("data-tooltip", tooltip);

}
async function get_targets() {
    return await fetch("/targets")
        .then(response => response.json());
}
async function init() {
    let targets = await get_targets();
    // sort targets by name
    targets.sort((a, b) => a.name.localeCompare(b.name));
    let children = [];
    for (let target of targets) {
        let status = await get_target_status(target);
        let target_dom = create_target_dom(target, status);
        children.push(target_dom);
        targets_by_id[target.id] = {
            target: target,
            dom: target_dom
        };
    }
    document.getElementById("main").append(...children);
}
init();
let event_source = new EventSource("/events");
event_source.onmessage = function(event) {
    let data = JSON.parse(event.data);
    if (data.type == "Observation") {
        let target = targets_by_id[data.monitoring_target.id];
        update_target_dom(target.dom, target.target, data.observed_status);
    }
};
let first_connection = true;
event_source.onopen = function(_) {
    if (!first_connection) {
        event_source.close();
        window.location.reload(true);
    }
    first_connection = false;
}
