package de.sve.backend.tasks;

import java.io.IOException;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.mail.MailAccount;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServlet;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;

public class CheckEmailConnectivity extends HttpServlet {

	private static final Logger LOG = LoggerFactory.getLogger(CheckEmailConnectivity.class);

	@Override
	protected void doGet(HttpServletRequest req, HttpServletResponse resp) throws ServletException, IOException {
		try {
			if (MailAccount.checkConnectivity()) {
				LOG.info("Email connectivity checks done successfully"); //$NON-NLS-1$
			} else {
				throw new ServletException("Email connectivity checks failed"); //$NON-NLS-1$
			}
		} catch (Exception e) {
			throw new ServletException("Email connectivity checks failed", e); //$NON-NLS-1$
		}
	}

}