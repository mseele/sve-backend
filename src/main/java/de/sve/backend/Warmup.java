package de.sve.backend;

import java.io.IOException;

import javax.servlet.ServletException;
import javax.servlet.http.HttpServlet;
import javax.servlet.http.HttpServletRequest;
import javax.servlet.http.HttpServletResponse;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.store.DataStore;

public class Warmup extends HttpServlet {

	private static final Logger LOG = LoggerFactory.getLogger(Warmup.class);

	@Override
	protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws ServletException, IOException {
		try {
			DataStore.events();
			LOG.info("Warm-up done"); //$NON-NLS-1$
		} catch (Exception e) {
			throw new ServletException("Warm-up failed", e); //$NON-NLS-1$
		}
	}

}