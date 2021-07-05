package de.sve.backend;

import org.eclipse.jetty.server.Server;
import org.eclipse.jetty.servlet.ServletContextHandler;
import org.eclipse.jetty.servlet.ServletHolder;
import org.glassfish.jersey.servlet.ServletContainer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.tasks.CheckEmailConnectivity;
import de.sve.backend.tasks.RenewCalendarWatch;

public class Application {

	private static final Logger LOG = LoggerFactory.getLogger(Application.class);

	public static void main(final String[] args) {
		// Create an instance of HttpServer bound to port defined by the
		// PORT environment variable when present, otherwise on 8080.
		final int port = Integer.parseInt(System.getenv().getOrDefault("PORT", "8080")); //$NON-NLS-1$ //$NON-NLS-2$
		final Server server = new Server(port);
		final ServletContextHandler ctx = new ServletContextHandler(ServletContextHandler.NO_SESSIONS);
		ctx.setContextPath("/"); //$NON-NLS-1$
		server.setHandler(ctx);

		// tasks
		ctx.addServlet(CheckEmailConnectivity.class, "/tasks/check_email_connectivity"); //$NON-NLS-1$
		ctx.addServlet(RenewCalendarWatch.class, "/tasks/renew_calendar_watch"); //$NON-NLS-1$

		// jersy handler
		final ServletHolder serHol = ctx.addServlet(ServletContainer.class, "/api/*"); //$NON-NLS-1$
		serHol.setInitOrder(1);
		serHol.setInitParameter("jersey.config.server.provider.packages", "de.sve.backend.api"); //$NON-NLS-1$ //$NON-NLS-2$

		try {
			server.start();
			server.join();
		} catch (final Exception ex) {
			LOG.error("Startup failed", ex); //$NON-NLS-1$
		} finally {
			server.destroy();
		}
	}

}
