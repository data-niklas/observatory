COLORS = {
  Healthy: "#5cdd8b",
  Unhealthy: "#dc3545",
  Degraded: "#ffc107",
};

class Route {
  constructor() {}
  build_html() {}
  on_attach() {}
  on_deattach() {}
  on_event(event) {}
}

class NotFoundRoute extends Route {
  build_html() {
    let main = document.createElement("main");
    main.innerHTML = "Route not found";
    return main;
  }
}

class Router {
  constructor(routes) {
    this.routes = routes;
    this.not_found = new NotFoundRoute();
    this.current_route = null;
    this.handle_route();
    window.onpopstate = window.onpushstate = this.handle_route.bind(this);
    let oldPushState = history.pushState;
    history.pushState = function pushState() {
      let ret = oldPushState.apply(this, arguments);
      window.onpushstate(new Event("pushstate"));
      return ret;
    };
  }

  handle_route() {
    let name = window.location.hash;
    if (name.startsWith("#")) {
      name = name.substring(1);
    }

    let route;
    if (name in this.routes) {
      route = this.routes[name];
    } else {
      route = this.not_found;
    }
    if (route == this.current_route) {
      return;
    }
    let new_html = route.build_html();
    if (this.current_route != null) this.current_route.on_deattach();
    let body = document.body;
    if (Array.isArray(new_html)) {
      body.replaceChildren(...new_html);
    } else {
      body.replaceChildren(new_html);
    }
    route.on_attach();
    this.current_route = route;
  }
}

class ObservatoryApiClient {
  static async target_status(target_id) {
    return await fetch("/status/" + target_id).then((response) =>
      response.json(),
    );
  }

  static async targets() {
    return await fetch("/targets").then((response) => response.json());
  }

  static async observations(target_id) {
    return await fetch("/observations/" + target_id).then((response) =>
      response.json(),
    );
  }
}

class OverviewRoute extends Route {
  constructor() {
    super();
    this.targets_by_id = {};
    this.loaded = false;
    this.children = [];
  }

  build_target_html(target, status) {
    let div = document.createElement("div");
    div.onclick = (_) => {
      let url = "/?id=" + target.id + "#details";
      history.pushState(null, null, url);
    };
    this.update_target_html(div, target, status);
    return div;
  }

  update_target_html(div, target, status) {
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
      color = COLORS[status.status];
    }
    div.children[0].children[0].innerHTML = target.name;
    div.children[0].children[1].innerHTML = status.description;
    div.children[1].style.backgroundColor = color;
    div.children[1].setAttribute("data-tooltip", tooltip);
  }

  build_html() {
    let main = document.createElement("main");
    main.id = "overview-main";
    return main;
  }

  async on_attach() {
    if (!this.loaded) {
      this.loaded = true;
      await this.init_children();
    }
    document.getElementById("overview-main").append(...this.children);
  }

  async refresh(){
    this.loaded = false;
    await this.on_attach();
  }

  async init_children() {
    let targets = await ObservatoryApiClient.targets();
    // sort targets by name
    targets.sort((a, b) => a.name.localeCompare(b.name));
    this.children = [];
    for (let target of targets) {
      let status = await ObservatoryApiClient.target_status(target.id);
      let target_dom = this.build_target_html(target, status);
      this.children.push(target_dom);
      this.targets_by_id[target.id] = {
        target: target,
        dom: target_dom,
      };
    }
  }

  async on_event(data) {
    if (data.type == "Observation") {
      if (!(data.monitoring_target.id in this.targets_by_id)) {
        await self.refresh();
      }
      let target = this.targets_by_id[data.monitoring_target.id];
      this.update_target_html(target.dom, target.target, data.observed_status);
    }
  }
}

class DetailsRoute extends Route {
  constructor() {
    super();
    this.target_id = null;
    this.observations = [];
  }

  build_html() {
    let header = document.createElement("header");
    header.id = "details-header";
    header.className = "horizontal";
    let main = document.createElement("main");
    main.id = "details-main";
    return [header, main];
  }

  async on_attach() {
    if (!location.search.startsWith("?id=") || location.search.includes("&")) {
      history.pushState(null, null, "#404");
    }
    this.target_id = location.search.substring(4);
    this.observations = await ObservatoryApiClient.observations(this.target_id);
    let header = document.getElementById("details-header");
    if (this.observations.length === 0) return;
    let target = this.observations[0].monitoring_target;
    let back_button = document.createElement("button");
    back_button.onclick = (_) => {
      history.back();
    };
    back_button.innerHTML = "<";
    let title = document.createElement("h2");
    title.innerHTML = target.name;
    header.appendChild(back_button);
    header.appendChild(title);
    let main = document.getElementById("details-main");
    let items = [];
    let current_day = null;
    for (let observation of this.observations) {
      let div = document.createElement("div");
      div.className = "horizontal observation";
      let status_div = document.createElement("div");
      status_div.className = "blob";
      status_div.style.backgroundColor =
        COLORS[observation.observed_status.status];
      let timestamp = moment.utc(observation.observed_status.timestamp);
      let formatted_timestamp = timestamp.format("YYYY-MM-DD HH:mm");
      status_div.setAttribute("data-tooltip", formatted_timestamp);
      status_div.setAttribute("data-placement", "right");

      let status_description = document.createElement("div");
      status_description.innerHTML = observation.observed_status.description;

      div.appendChild(status_div);
      div.appendChild(status_description);
      if (current_day == null || !current_day.isSame(timestamp, "day")){
        current_day = timestamp;
        let formatted_day = timestamp.format("YYYY-MM-DD");
        let day = document.createElement("div");
        day.innerHTML = formatted_day;
        day.className = "datetime";
        div.appendChild(day);
      }
      items.push(div);
    }
    main.replaceChildren(...items);
  }
}

let router = new Router({
  "": new OverviewRoute(),
  details: new DetailsRoute(),
});

let event_source = new EventSource("/events");
event_source.onmessage = function (event) {
  let data = JSON.parse(event.data);
  router.current_route.on_event(data);
};

let first_connection = true;
event_source.onopen = function (_) {
  if (!first_connection) {
    event_source.close();
    window.location.reload(true);
  }
  first_connection = false;
};
