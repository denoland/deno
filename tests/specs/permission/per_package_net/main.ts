import { listenOn as webserverListen } from "./node_modules/webserver/index.js";
import { listenOn as widgetListen } from "./node_modules/widget/index.js";

// npm:webserver is granted net access to :45999 only.
try {
  console.log("webserver:", webserverListen(45999));
} catch (e) {
  console.log("webserver :45999 DENIED:", (e as Error).message);
}

// Same package, a port it was not granted: denied.
try {
  console.log("webserver:", webserverListen(46000));
} catch (e) {
  console.log("webserver :46000 DENIED:", (e as Error).message);
}

// npm:widget has no net grant: denied even on the port granted to webserver,
// showing grants are scoped per package.
try {
  console.log("widget:", widgetListen(45999));
} catch (e) {
  console.log("widget :45999 DENIED:", (e as Error).message);
}
