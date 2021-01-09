package de.sve.backend.tasks;

import java.io.IOException;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.manager.CalendarManager;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServlet;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;

public class RenewCalendarWatch extends HttpServlet {

	private static final Logger LOG = LoggerFactory.getLogger(RenewCalendarWatch.class);

	@Override
	protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws ServletException, IOException {
		try {
			LOG.info("Calendar watch has been renewed: " + CalendarManager.renewWatch()); //$NON-NLS-1$
		} catch (Exception e) {
			throw new ServletException("Error renewing calendar watch", e); //$NON-NLS-1$
		}
	}

}