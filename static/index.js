let targets_by_name = {};
    async function get_target_status(target) {
      return await fetch("/status/" + target.name)
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
        let status_line = document.createElement("div");
        div.appendChild(title);
        div.appendChild(status_line);
      }

      let tooltip = "";
      let color = "";
      if (status === null){
        tooltip = "No data";
        color = "#ffc107";
      }
      else {
        let timestamp = new Date(status.timestamp);
      let formatted_timestamp = timestamp.toISOString().replace("T", " ").replace(/\.\d+Z/, "").split(":").slice(0, 2).join(":");
        tooltip = "Last checked: " + formatted_timestamp;
        color = {
          "Healthy": "#5cdd8b",
          "Unhealthy": "#dc3545"
        }[status.status];
      }

      div.children[0].innerHTML = target.name;
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
        targets_by_name[target.name] = {
          target: target,
          dom: target_dom
        };
      }
      document.getElementById("targets").append(...children);
    }
    init();
    let event_source = new EventSource("/events");
    event_source.onmessage = function (event) {
      let data = JSON.parse(event.data);
      let div = document.createElement("div");
      let target = targets_by_name[data.monitoring_target.name];
      update_target_dom(target.dom, target.target, data.observed_status);
    };
